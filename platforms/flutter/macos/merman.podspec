Pod::Spec.new do |s|
  s.name             = 'merman'
  s.version          = '0.8.0-alpha.2'
  s.summary          = 'Flutter FFI bindings for headless Mermaid rendering on macOS.'
  s.description      = <<-DESC
    Provides a Flutter plugin that links the merman Rust FFI library and exposes
    it to Dart FFI via DynamicLibrary.process().
  DESC
  s.homepage         = 'https://github.com/Latias94/merman'
  s.license          = { :type => 'MIT' }
  s.author           = { 'Merman' => 'https://github.com/Latias94/merman' }
  s.source           = { :path => '.' }

  s.platform         = :osx, '11.0'
  s.swift_version    = '5.7'

  s.source_files     = 'merman/Sources/merman/**/*.swift'
  s.dependency       'FlutterMacOS'
  s.vendored_libraries = 'Libraries/libmerman_ffi.dylib'

  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
  }
end
