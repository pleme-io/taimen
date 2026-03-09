# Taimen home-manager module — open-source video conferencing server
#
# Namespace: services.taimen
#
# Module factory: receives { hmHelpers } from flake.nix, returns HM module.
{ hmHelpers }:
{
  lib,
  config,
  pkgs,
  ...
}:
with lib;
let
  cfg = config.services.taimen;
in
{
  options.services.taimen = {
    enable = mkOption {
      type = types.bool;
      default = false;
      description = "Enable the taimen video conferencing server.";
    };
    package = mkOption {
      type = types.package;
      default = pkgs.taimen;
      description = "The taimen package to install.";
    };
  };
  config = mkIf cfg.enable {
    home.packages = [ cfg.package ];
  };
}
