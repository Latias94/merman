import 'dart:convert';
import 'dart:io';

import 'package:code_assets/code_assets.dart';
import 'package:crypto/crypto.dart';
import 'package:hooks/hooks.dart';

import '../hook/build.dart' as hook;
import '../lib/src/generated/package_version.dart';

void expect(bool condition, String message) {
  if (!condition) throw StateError(message);
}

Future<void> rejects(Future<void> Function() action, String message) async {
  try {
    await action();
  } catch (error) {
    expect(error.toString().contains(message), 'expected $message, got $error');
    return;
  }
  throw StateError('expected failure containing $message');
}

Future<void> main() async {
  final root = await Directory.systemTemp.createTemp('merman-hook-contract-');
  final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
  final requests = <String>[];
  var corruptDownload = false;
  final bytes = utf8.encode('tiny native fixture, never loaded');
  final compressed = gzip.encode(bytes);
  server.listen((request) async {
    requests.add(request.uri.path);
    if (request.uri.path.startsWith('/stall-redirect/')) {
      request.response
        ..statusCode = HttpStatus.found
        ..headers.set(HttpHeaders.locationHeader, '/stall/asset.gz');
      await request.response.close();
      return;
    }
    if (request.uri.path.startsWith('/stall/')) return;
    if (request.uri.path.startsWith('/redirect/')) {
      request.response
        ..statusCode = HttpStatus.found
        ..headers.set(
          HttpHeaders.locationHeader,
          '/signed${request.uri.path.substring('/redirect'.length)}?token=fixture',
        );
      await request.response.close();
      return;
    }
    request.response.add(
      corruptDownload ? List.filled(compressed.length, 0) : compressed,
    );
    await request.response.close();
  });
  final package = Directory.fromUri(root.uri.resolve('package/'))..createSync();
  final manifestFile = File.fromUri(
    package.uri.resolve('hook/native-assets.json'),
  );
  manifestFile.parent.createSync();

  BuildInput input({
    String cache = 'cache/',
    OS os = OS.macOS,
    Architecture arch = Architecture.arm64,
    IOSSdk? sdk,
    Map<String, Object?> defines = const {},
    int androidApi = 24,
    bool code = true,
  }) {
    final builder = BuildInputBuilder()
      ..setupShared(
        packageRoot: package.uri,
        packageName: 'merman',
        outputDirectoryShared: root.uri.resolve(cache),
        outputFile: root.uri.resolve('output.json'),
        userDefines: PackageUserDefines(
          workspacePubspec: PackageUserDefinesSource(
            defines: defines,
            basePath: root.uri,
          ),
        ),
      );
    if (code) {
      builder.config.setupBuild(linkingEnabled: false);
      builder.addExtension(
        CodeAssetExtension(
          targetArchitecture: arch,
          targetOS: os,
          linkModePreference: LinkModePreference.dynamic,
          iOS: os == OS.iOS
              ? IOSCodeConfig(targetSdk: sdk!, targetVersion: 13)
              : null,
          android: os == OS.android
              ? AndroidCodeConfig(targetNdkApi: androidApi)
              : null,
          macOS: os == OS.macOS ? MacOSCodeConfig(targetVersion: 11) : null,
        ),
      );
    }
    return builder.build();
  }

  void manifest() {
    final targets = <String, Object?>{};
    for (final target in [
      'android/armeabi-v7a',
      'android/arm64-v8a',
      'android/x86_64',
      'ios/arm64',
      'ios/arm64-simulator',
      'ios/x86_64-simulator',
      'macos/arm64',
      'macos/x86_64',
      'linux/aarch64',
      'linux/x86_64',
      'windows/x86_64',
    ]) {
      final asset =
          'merman-flutter-native-$mermanPackageVersion-${target.replaceAll('/', '-')}.gz';
      final library = target.startsWith('windows/')
          ? 'merman_ffi.dll'
          : target.startsWith('macos/') || target.startsWith('ios/')
          ? 'libmerman_ffi.dylib'
          : 'libmerman_ffi.so';
      targets[target] = {
        'asset': asset,
        'library': library,
        'url': 'https://example.invalid/$asset',
        'sha256': sha256.convert(compressed).toString(),
        'size': compressed.length,
        'library_sha256': sha256.convert(bytes).toString(),
        'library_size': bytes.length,
      };
    }
    manifestFile.writeAsStringSync(
      jsonEncode({
        'schema_version': 1,
        'package': 'merman',
        'version': mermanPackageVersion,
        'source_sha': '1' * 40,
        'source_tree': '2' * 40,
        'targets': targets,
      }),
    );
  }

  final mirror = 'http://127.0.0.1:${server.port}/';
  try {
    manifest();
    final configFile = File.fromUri(root.uri.resolve('input.json'));
    configFile.writeAsStringSync(
      jsonEncode(input(defines: {'mirror': mirror}).json),
    );
    await hook.main(['--config', configFile.path]);
    final result = BuildOutput(
      jsonDecode(
            File.fromUri(root.uri.resolve('output.json')).readAsStringSync(),
          )
          as Map<String, Object?>,
    );
    final asset = result.assets.code.single;
    expect(
      asset.id == 'package:merman/src/generated/native_abi.dart',
      'asset id changed',
    );
    expect(
      asset.linkMode is DynamicLoadingBundled,
      'must bundle native library',
    );
    expect(
      File.fromUri(asset.file!).readAsBytesSync().toString() ==
          bytes.toString(),
      'wrong target bytes',
    );
    expect(
      requests.single.endsWith('-macos-arm64.gz'),
      'must fetch only selected target',
    );
    expect(
      result.dependencies.contains(manifestFile.uri),
      'manifest dependency missing',
    );
    expect(
      result.dependencies.contains(asset.file),
      'library cache dependency missing',
    );

    await hook.buildMerman(
      input(defines: {'offline': true}),
      BuildOutputBuilder(),
    );
    expect(requests.length == 1, 'warm offline must not use network');
    await rejects(
      () => hook.buildMerman(
        input(cache: 'cold/', defines: {'offline': true}),
        BuildOutputBuilder(),
      ),
      'offline cache is missing',
    );

    // Both compressed and materialized cache bytes are rechecked on every invocation.
    final library = File.fromUri(asset.file!);
    library.writeAsBytesSync(List.filled(bytes.length, 0));
    await rejects(
      () => hook.buildMerman(
        input(defines: {'offline': true}),
        BuildOutputBuilder(),
      ),
      'digest/size mismatch',
    );
    library.writeAsBytesSync(bytes);
    final cachedGzip = File.fromUri(library.parent.uri.resolve('library.gz'));
    cachedGzip.writeAsBytesSync(List.filled(compressed.length, 0));
    await rejects(
      () => hook.buildMerman(
        input(defines: {'mirror': mirror}),
        BuildOutputBuilder(),
      ),
      'digest/size mismatch',
    );
    expect(requests.length == 1, 'bad cache must fail visibly');

    corruptDownload = true;
    await rejects(
      () => hook.buildMerman(
        input(cache: 'bad-download/', defines: {'mirror': mirror}),
        BuildOutputBuilder(),
      ),
      'digest/size mismatch',
    );
    corruptDownload = false;
    final failedCache = Directory.fromUri(root.uri.resolve('bad-download/'));
    expect(
      failedCache.listSync(recursive: true).whereType<File>().isEmpty,
      'failed downloads must not leave accepted or partial files',
    );

    await hook.buildMerman(
      input(cache: 'redirected/', defines: {'mirror': '${mirror}redirect/'}),
      BuildOutputBuilder(),
    );
    expect(
      requests.last.startsWith('/signed/'),
      'signed HTTPS-host redirects must be supported',
    );

    final wrongVersion = jsonDecode(manifestFile.readAsStringSync()) as Map;
    wrongVersion['version'] = '0.0.0';
    manifestFile.writeAsStringSync(jsonEncode(wrongVersion));
    await rejects(
      () => hook.buildMerman(input(), BuildOutputBuilder()),
      'manifest does not match',
    );
    manifest();
    await rejects(
      () => hook.buildMerman(
        input(defines: {'offline': 'true'}),
        BuildOutputBuilder(),
      ),
      'offline must be a boolean',
    );
    await rejects(
      () => hook.buildMerman(
        input(defines: {'mirror': 'http://example.org/'}),
        BuildOutputBuilder(),
      ),
      'require HTTPS',
    );

    for (final (os, arch, sdk, target) in [
      (OS.iOS, Architecture.arm64, IOSSdk.iPhoneOS, 'ios-arm64'),
      (
        OS.iOS,
        Architecture.arm64,
        IOSSdk.iPhoneSimulator,
        'ios-arm64-simulator',
      ),
      (
        OS.iOS,
        Architecture.x64,
        IOSSdk.iPhoneSimulator,
        'ios-x86_64-simulator',
      ),
      (OS.android, Architecture.arm, null, 'android-armeabi-v7a'),
      (OS.android, Architecture.arm64, null, 'android-arm64-v8a'),
      (OS.android, Architecture.x64, null, 'android-x86_64'),
      (OS.macOS, Architecture.x64, null, 'macos-x86_64'),
      (OS.linux, Architecture.arm64, null, 'linux-aarch64'),
      (OS.linux, Architecture.x64, null, 'linux-x86_64'),
      (OS.windows, Architecture.x64, null, 'windows-x86_64'),
    ]) {
      await hook.buildMerman(
        input(
          cache: '$target/',
          os: os,
          arch: arch,
          sdk: sdk,
          defines: {'mirror': mirror},
        ),
        BuildOutputBuilder(),
      );
      expect(
        requests.last.endsWith('-$target.gz'),
        'incorrect SDK/target selection',
      );
    }
    await rejects(
      () => hook.buildMerman(
        input(os: OS.android, androidApi: 23),
        BuildOutputBuilder(),
      ),
      'API 24',
    );
    await rejects(
      () => hook.buildMerman(
        input(os: OS.windows, arch: Architecture.arm64),
        BuildOutputBuilder(),
      ),
      'Unsupported',
    );

    final override = File.fromUri(
      root.uri.resolve('prepared/macos/arm64/libmerman_ffi.dylib'),
    );
    override.parent.createSync(recursive: true);
    override.writeAsStringSync('explicit local custom library');
    final beforeLocal = requests.length;
    final localOutput = BuildOutputBuilder();
    await hook.buildMerman(
      input(defines: {'native_assets': 'prepared', 'offline': true}),
      localOutput,
    );
    final localResult = BuildOutput(localOutput.json);
    expect(
      localResult.assets.code.single.file == override.uri,
      'local override did not participate in bundling',
    );
    expect(
      localResult.dependencies.contains(override.uri),
      'local dependency missing',
    );
    expect(requests.length == beforeLocal, 'local override used network');
    await rejects(
      () => hook.buildMerman(
        input(defines: {'native_assets': 'missing'}),
        BuildOutputBuilder(),
      ),
      'explicit native_assets override',
    );

    manifestFile.deleteSync();
    final developer = File.fromUri(
      package.uri.resolve('native/macos/arm64/libmerman_ffi.dylib'),
    );
    developer.parent.createSync(recursive: true);
    developer.writeAsBytesSync(bytes);
    await hook.buildMerman(input(), BuildOutputBuilder());
    expect(
      requests.length == beforeLocal,
      'self-contained development used network',
    );
    await hook.buildMerman(input(code: false), BuildOutputBuilder());
    manifest();
    await rejects(
      () => hook.buildMerman(
        input(
          cache: 'headers-timeout/',
          defines: {'mirror': '${mirror}stall-redirect/'},
        ),
        BuildOutputBuilder(),
      ),
      'TimeoutException',
    );
    expect(
      requests.contains('/stall/asset.gz'),
      'hanging redirect target was not requested',
    );
    expect(
      !root
          .listSync(recursive: true)
          .any((entry) => entry.path.endsWith('.part')),
      'timeout left partial cache bytes',
    );
    stdout.writeln(
      'native assets contracts passed (HTTP, cache, offline, local, all targets/SDKs)',
    );
  } finally {
    await server.close(force: true);
    await root.delete(recursive: true);
  }
}
