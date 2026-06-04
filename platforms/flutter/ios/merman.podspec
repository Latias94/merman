Pod::Spec.new do |s|
  s.name             = 'merman'
  s.version          = '0.7.0-alpha.1'
  s.summary          = 'Flutter FFI bindings for headless Mermaid rendering.'
  s.description      = <<-DESC
    Provides a Flutter plugin that links the merman Rust FFI framework and
    exposes it to Dart FFI via DynamicLibrary.process().
  DESC
  s.homepage         = 'https://github.com/Latias94/merman'
  s.license          = { :type => 'MIT' }
  s.author           = { 'Merman' => 'https://github.com/Latias94/merman' }
  s.source           = { :path => '.' }

  s.platform         = :ios, '13.0'
  s.swift_version    = '5.7'

  s.source_files     = 'merman/Sources/merman/**/*.swift'
  s.dependency       'Flutter'
  s.vendored_frameworks = 'MermanFFI.xcframework'

  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
    'EXCLUDED_ARCHS[sdk=iphonesimulator*]' => 'i386',
  }
end
