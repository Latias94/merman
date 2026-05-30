param(
    [string[]] $Targets = @("aarch64-linux-android", "x86_64-linux-android"),
    [string] $ProjectName = "merman_smoke"
)

$ErrorActionPreference = "Stop"

$pluginRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$repoRoot = Resolve-Path (Join-Path $pluginRoot "..\..")
$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) "$ProjectName-$([guid]::NewGuid().ToString('N'))"

Write-Host "Building Android native slices for Flutter plugin smoke"
& (Join-Path $repoRoot "platforms\android\build-android.ps1") -Targets $Targets -Profile release

Write-Host "Creating temporary Flutter app: $tempRoot"
flutter create --platforms android --project-name $ProjectName $tempRoot | Out-Host

$pubspec = Join-Path $tempRoot "pubspec.yaml"
Add-Content -LiteralPath $pubspec -Value @"

dependency_overrides:
  merman:
    path: $($pluginRoot.Path.Replace('\', '/'))
"@

$main = Join-Path $tempRoot "lib\main.dart"
Set-Content -LiteralPath $main -Value @'
import 'package:flutter/material.dart';
import 'package:merman/merman.dart';

void main() {
  runApp(const SmokeApp());
}

class SmokeApp extends StatelessWidget {
  const SmokeApp({super.key});

  @override
  Widget build(BuildContext context) {
    final version = Merman.open().packageVersion;
    return MaterialApp(
      home: Scaffold(
        body: Center(child: Text('merman $version')),
      ),
    );
  }
}
'@

Push-Location $tempRoot
try {
    flutter pub get | Out-Host
    flutter build apk --debug | Out-Host
}
finally {
    Pop-Location
}

Write-Host "Flutter Android smoke app built at $tempRoot"
