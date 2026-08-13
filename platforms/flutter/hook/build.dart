import 'dart:io';

import 'package:code_assets/code_assets.dart';
import 'package:hooks/hooks.dart';

const _assetName = 'src/generated/native_abi.dart';

void main(List<String> args) async {
  await build(args, (input, output) async {
    if (!input.config.buildCodeAssets) {
      return;
    }

    final config = input.config.code;
    if (config.targetOS == OS.android && config.android.targetNdkApi < 24) {
      throw UnsupportedError(
        'Merman requires Android API 24 or newer; the application targets '
        'API ${config.android.targetNdkApi}.',
      );
    }
    final relativePath = _libraryPath(
      config.targetOS,
      config.targetArchitecture,
      iosSdk: config.targetOS == OS.iOS ? config.iOS.targetSdk : null,
    );
    final library = input.packageRoot.resolve(relativePath);
    if (!File.fromUri(library).existsSync()) {
      throw StateError(
        'Merman has no prebuilt native library for '
        '${config.targetOS.name}/${config.targetArchitecture.name}: '
        '${library.toFilePath()}',
      );
    }

    output.dependencies.add(library);
    output.assets.code.add(
      CodeAsset(
        package: input.packageName,
        name: _assetName,
        linkMode: DynamicLoadingBundled(),
        file: library,
      ),
    );
  });
}

String _libraryPath(OS os, Architecture architecture, {IOSSdk? iosSdk}) {
  final arch = architecture.name;
  if (os == OS.android) {
    final abi = switch (arch) {
      'arm' => 'armeabi-v7a',
      'arm64' => 'arm64-v8a',
      'x64' => 'x86_64',
      _ => throw UnsupportedError('Unsupported Android architecture: $arch'),
    };
    return 'native/android/$abi/libmerman_ffi.so';
  }
  if (os == OS.iOS) {
    final sdk = iosSdk?.type;
    final slice = switch ((sdk, arch)) {
      ('iphoneos', 'arm64') => 'arm64',
      ('iphonesimulator', 'arm64') => 'arm64-simulator',
      ('iphonesimulator', 'x64') => 'x86_64-simulator',
      _ => throw UnsupportedError('Unsupported iOS target: $sdk/$arch'),
    };
    return 'native/ios/$slice/libmerman_ffi.dylib';
  }
  if (os == OS.macOS) {
    return switch (arch) {
      'arm64' => 'native/macos/arm64/libmerman_ffi.dylib',
      'x64' => 'native/macos/x86_64/libmerman_ffi.dylib',
      _ => throw UnsupportedError('Unsupported macOS architecture: $arch'),
    };
  }
  if (os == OS.linux) {
    return switch (arch) {
      'arm64' => 'native/linux/aarch64/libmerman_ffi.so',
      'x64' => 'native/linux/x86_64/libmerman_ffi.so',
      _ => throw UnsupportedError('Unsupported Linux architecture: $arch'),
    };
  }
  if (os == OS.windows && arch == 'x64') {
    return 'native/windows/x86_64/merman_ffi.dll';
  }
  throw UnsupportedError('Unsupported native target: ${os.name}/$arch');
}
