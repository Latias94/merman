import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:crypto/crypto.dart';
import 'package:hooks/hooks.dart';

import '../lib/src/generated/package_version.dart';

const nativeManifestPath = 'hook/native-assets.json';

Future<Uri> acquireNativeLibrary(
  BuildInput input,
  BuildOutputBuilder output,
  String relativePath,
) async {
  final local = input.userDefines['native_assets'];
  if (local != null) {
    if (local is! String || local.isEmpty) {
      throw const FormatException(
        'merman native_assets must be a directory path',
      );
    }
    final root = Directory.fromUri(input.userDefines.path('native_assets')!);
    final library = root.uri.resolve(relativePath.substring('native/'.length));
    _requireFile(library, 'explicit native_assets override');
    output.dependencies.add(library);
    return library;
  }

  final manifestFile = File.fromUri(
    input.packageRoot.resolve(nativeManifestPath),
  );
  if (!manifestFile.existsSync()) {
    final library = input.packageRoot.resolve(relativePath);
    _requireFile(
      library,
      'self-contained development package (build native assets explicitly or '
      'set hooks.user_defines.merman.native_assets)',
    );
    output.dependencies.add(library);
    return library;
  }
  output.dependencies.add(manifestFile.uri);
  final manifest = jsonDecode(await manifestFile.readAsString()) as Map;
  if (manifest['schema_version'] != 1 ||
      manifest['package'] != 'merman' ||
      manifest['version'] != mermanPackageVersion) {
    throw StateError('Merman native manifest does not match this Dart package');
  }
  final parts = relativePath.split('/');
  final target = '${parts[1]}/${parts[2]}';
  final entry = (manifest['targets'] as Map)[target];
  if (entry is! Map || entry['library'] != parts.last) {
    throw UnsupportedError('Merman has no pinned native asset for $target');
  }
  final asset = entry['asset'];
  if (asset !=
      'merman-flutter-native-$mermanPackageVersion-${target.replaceAll('/', '-')}.gz') {
    throw const FormatException('Invalid Merman native asset name');
  }
  for (final field in ['sha256', 'library_sha256']) {
    if (entry[field] is! String ||
        !RegExp(r'^[0-9a-f]{64}$').hasMatch(entry[field] as String)) {
      throw FormatException('Invalid Merman native $field');
    }
  }
  for (final field in ['size', 'library_size']) {
    if (entry[field] is! int || (entry[field] as int) <= 0) {
      throw FormatException('Invalid Merman native $field');
    }
  }
  final offline = input.userDefines['offline'];
  if (offline != null && offline is! bool) {
    throw const FormatException('merman offline must be a boolean');
  }
  final mirror = input.userDefines['mirror'];
  if (mirror != null && (mirror is! String || mirror.isEmpty)) {
    throw const FormatException('merman mirror must be an HTTPS base URL');
  }
  var url = Uri.parse(entry['url'] as String);
  _requireDownloadUrl(url);
  if (mirror is String) {
    final base = Uri.parse(mirror.endsWith('/') ? mirror : '$mirror/');
    _requireDownloadUrl(base);
    url = base.resolve(asset as String);
  }

  // Hooks serializes this hook's shared output directory, including across targets.
  final cache = Directory.fromUri(
    input.outputDirectoryShared.resolve('native/${entry['sha256']}/'),
  );
  await cache.create(recursive: true);
  final compressed = File.fromUri(cache.uri.resolve('library.gz'));
  if (!compressed.existsSync()) {
    if (offline == true) {
      throw StateError(
        'Merman offline cache is missing $target. Build once online with this '
        'package version, preserve .dart_tool hooks outputs, or set '
        'hooks.user_defines.merman.native_assets to a prepared native directory.',
      );
    }
    await _download(
      url,
      compressed,
      entry['sha256'] as String,
      entry['size'] as int,
    );
  }
  await _verify(compressed, entry['sha256'] as String, entry['size'] as int);
  output.dependencies.add(compressed.uri);

  final library = File.fromUri(cache.uri.resolve(parts.last));
  if (!library.existsSync()) {
    final temporary = File('${library.path}.part');
    try {
      await _writeBounded(
        gzip.decoder.bind(compressed.openRead()),
        temporary,
        entry['library_size'] as int,
      );
      await _verify(
        temporary,
        entry['library_sha256'] as String,
        entry['library_size'] as int,
      );
      await temporary.rename(library.path);
    } finally {
      if (temporary.existsSync()) await temporary.delete();
    }
  }
  await _verify(
    library,
    entry['library_sha256'] as String,
    entry['library_size'] as int,
  );
  output.dependencies.add(library.uri);
  return library.uri;
}

void _requireFile(Uri uri, String source) {
  if (!File.fromUri(uri).existsSync()) {
    throw StateError('Merman native library is missing from $source: $uri');
  }
}

void _requireDownloadUrl(Uri url, {bool redirect = false}) {
  final local = ['127.0.0.1', 'localhost', '::1'].contains(url.host);
  if (url.host.isEmpty ||
      url.userInfo.isNotEmpty ||
      (!redirect && url.hasQuery) ||
      url.hasFragment ||
      (url.scheme != 'https' && !(url.scheme == 'http' && local))) {
    throw const FormatException(
      'Merman native downloads require HTTPS (HTTP is allowed for loopback fixtures)',
    );
  }
}

Future<void> _verify(File file, String expected, int size) async {
  if (await file.length() != size ||
      (await sha256.bind(file.openRead()).first).toString() != expected) {
    throw StateError(
      'Merman native digest/size mismatch: ${file.path}. Remove the invalid '
      'hook cache file and rebuild, or use an explicit native_assets override.',
    );
  }
}

Future<void> _download(
  Uri url,
  File destination,
  String digest,
  int size,
) async {
  final client = HttpClient()
    ..autoUncompress = false
    ..connectionTimeout = const Duration(seconds: 30)
    ..findProxy = HttpClient.findProxyFromEnvironment;
  final temporary = File('${destination.path}.part');
  try {
    var current = url;
    for (var redirects = 0; ; redirects++) {
      _requireDownloadUrl(current, redirect: redirects > 0);
      final request = await client.getUrl(current);
      request.followRedirects = false;
      final response = await request.close().timeout(
        const Duration(seconds: 60),
      );
      if (response.isRedirect && redirects < 5) {
        final location = response.headers.value(HttpHeaders.locationHeader);
        if (location == null) {
          throw HttpException('Missing redirect location', uri: current);
        }
        await response.timeout(const Duration(seconds: 60)).drain<void>();
        current = current.resolve(location);
        continue;
      }
      if (response.statusCode != HttpStatus.ok) {
        throw HttpException(
          'Merman native download HTTP ${response.statusCode}',
          uri: current,
        );
      }
      await _writeBounded(
        response.timeout(const Duration(seconds: 60)),
        temporary,
        size,
      );
      await _verify(temporary, digest, size);
      await temporary.rename(destination.path);
      return;
    }
  } catch (error) {
    if (error is! IOException && error is! TimeoutException) rethrow;
    throw StateError(
      'Merman native download failed for $url: $error. '
      'Prepare the hook cache online, set mirror, or supply native_assets.',
    );
  } finally {
    client.close(force: true);
    if (temporary.existsSync()) await temporary.delete();
  }
}

Future<void> _writeBounded(
  Stream<List<int>> stream,
  File file,
  int size,
) async {
  var received = 0;
  final sink = file.openWrite();
  try {
    await sink.addStream(
      stream.map((chunk) {
        received += chunk.length;
        if (received > size) {
          throw StateError('Merman native asset exceeds pinned size');
        }
        return chunk;
      }),
    );
    if (received != size) {
      throw StateError('Merman native asset is shorter than pinned size');
    }
  } finally {
    await sink.close();
  }
}
