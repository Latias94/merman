{
  lib,
  rustPlatform,
  python3,
}:

let
  repositoryRoot = ../.;
  workspace = lib.importTOML (repositoryRoot + "/Cargo.toml");
  descriptor = builtins.fromJSON (
    builtins.readFile (repositoryRoot + "/capabilities/artifact-profiles-v1.json")
  );
  matchingProfiles = builtins.filter (candidate: candidate.id == "cli-release") descriptor.profiles;
  profile =
    if builtins.length matchingProfiles == 1 then
      builtins.head matchingProfiles
    else
      throw "Merman Nix package requires exactly one cli-release artifact profile";

  sourcePolicy = builtins.fromJSON (builtins.readFile ./source-policy.json);
  allowedTrees = sourcePolicy.root_directories ++ workspace.workspace.members;
  rootString = toString repositoryRoot;
  sourceFilter =
    path: type:
    let
      pathString = toString path;
      relative = if pathString == rootString then "" else lib.removePrefix "${rootString}/" pathString;
      segments = lib.filter (segment: segment != "") (lib.splitString "/" relative);
      top = if segments == [ ] then "" else builtins.head segments;
      directorySegments = if type == "directory" then segments else lib.init segments;
      allowedTree = lib.any (
        tree: relative == tree || lib.hasPrefix "${tree}/" relative || lib.hasPrefix "${relative}/" tree
      ) allowedTrees;
      allowedByRoot =
        if segments == [ ] then
          true
        else if builtins.length segments == 1 then
          builtins.elem relative sourcePolicy.root_files || allowedTree || relative == "scripts"
        else if top == "scripts" then
          builtins.elem relative sourcePolicy.script_files
        else
          allowedTree;
      excludedDirectory = lib.any (
        segment: builtins.elem segment sourcePolicy.excluded_directory_names
      ) directorySegments;
      excludedFile =
        type != "directory" && builtins.elem (baseNameOf path) sourcePolicy.excluded_file_names;
    in
    type != "symlink" && allowedByRoot && !excludedDirectory && !excludedFile;
  filteredSource = lib.cleanSourceWith {
    name = "merman-cli-source";
    src = repositoryRoot;
    filter = sourceFilter;
  };
in
assert profile.cargo.package == "merman-cli";
assert profile.cargo.manifest == "crates/merman-cli/Cargo.toml";
assert profile.cargo.profile == "dist";
assert profile.cargo.default_features == false;
assert profile.cargo.target.name == "merman-cli";
assert profile.cargo.target.kinds == [ "bin" ];
rustPlatform.buildRustPackage rec {
  pname = "merman-cli";
  version = workspace.workspace.package.version;

  src = filteredSource;
  cargoLock.lockFile = filteredSource + "/Cargo.lock";

  buildType = profile.cargo.profile;
  buildNoDefaultFeatures = !profile.cargo.default_features;
  buildFeatures = profile.cargo.features;
  cargoBuildFlags = [
    "--locked"
    "--package"
    profile.cargo.package
    "--bin"
    profile.cargo.target.name
  ];

  AWS_LC_SYS_USE_SYSTEM = "0";

  doCheck = false;

  postInstall = ''
    completion_source=crates/merman-cli/assets/completions
    install -Dm0644 "$completion_source/merman-cli.bash" \
      "$out/share/bash-completion/completions/merman-cli"
    install -Dm0644 "$completion_source/_merman-cli" \
      "$out/share/zsh/site-functions/_merman-cli"
    install -Dm0644 "$completion_source/merman-cli.fish" \
      "$out/share/fish/vendor_completions.d/merman-cli.fish"
    install -Dm0644 "$completion_source/merman-cli.ps1" \
      "$out/share/pwsh/completions/_merman-cli.ps1"
    install -Dm0644 "$completion_source/merman-cli.elv" \
      "$out/share/elvish/lib/merman-cli.elv"

    for manpage in crates/merman-cli/assets/man/*.1; do
      install -Dm0644 "$manpage" "$out/share/man/man1/$(basename "$manpage")"
    done

    doc_dir="$out/share/doc/merman-cli"
    mkdir -p "$doc_dir"
    install -m 0644 LICENSE-APACHE LICENSE-MIT THIRD_PARTY_NOTICES.md "$doc_dir/"
    cp -R THIRD_PARTY_LICENSES "$doc_dir/"
  '';

  doInstallCheck = true;
  nativeInstallCheckInputs = [ python3 ];
  installCheckPhase = ''
    runHook preInstallCheck
    ${lib.getExe python3} ${filteredSource}/scripts/verify_cli_installation.py \
      --package-version ${lib.escapeShellArg version} \
      --prefix "$out" \
      --binary "$out/bin/merman-cli" \
      --contract-root ${filteredSource} \
      --completion-layout nix
    cmp ${filteredSource}/LICENSE-APACHE "$out/share/doc/merman-cli/LICENSE-APACHE"
    cmp ${filteredSource}/LICENSE-MIT "$out/share/doc/merman-cli/LICENSE-MIT"
    cmp ${filteredSource}/THIRD_PARTY_NOTICES.md \
      "$out/share/doc/merman-cli/THIRD_PARTY_NOTICES.md"
    diff -qr ${filteredSource}/THIRD_PARTY_LICENSES \
      "$out/share/doc/merman-cli/THIRD_PARTY_LICENSES"
    runHook postInstallCheck
  '';

  passthru = {
    artifactProfile = profile;
    source = filteredSource;
    inherit sourcePolicy;
  };

  meta = {
    description = "Headless Mermaid-compatible diagram CLI";
    homepage = workspace.workspace.package.homepage;
    license = with lib.licenses; [
      asl20
      mit
    ];
    mainProgram = "merman-cli";
    platforms = [
      "aarch64-darwin"
      "aarch64-linux"
      "x86_64-darwin"
      "x86_64-linux"
    ];
  };
}
