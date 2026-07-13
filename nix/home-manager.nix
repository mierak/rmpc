# Extends the `programs.rmpc` module that ships with Home Manager
# (enable/package/config) with structured `settings` and `themes` options
# rendered to RON. The rendered settings flow through the upstream `config`
# option, which is `types.lines` and concatenates all definitions — so set
# either `settings` or `config`, never both.
{
  config,
  lib,
  ...
}: let
  cfg = config.programs.rmpc;
  ron = import ./ron.nix {inherit lib;};

  # Shorthands for shapes that appear in almost every rmpc config, exposed
  # together with the ron.nix constructors as the `rmpcLib` module argument.
  rmpcLib =
    ron
    // {
      # pane "Queue" -> Pane(Queue)
      pane = name: ron.enum "Pane" [(ron.variant name)];

      # tab "Albums" "Albums" -> (name: "Albums", pane: Pane(Albums))
      tab = name: pane: {
        inherit name;
        pane = rmpcLib.pane pane;
      };

      # prop (variant "Artist") -> Property(Artist)
      prop = kind: ron.enum "Property" [kind];

      # text " - " -> Text(" - ")
      text = s: ron.enum "Text" [s];
    };

  # Tagged values (variant/enum/struct/raw) are attrsets, admitted by the
  # attrsOf branch.
  ronValue = let
    t = lib.types;
    valueType =
      t.nullOr (t.oneOf [
        t.bool
        t.int
        t.float
        t.str
        (t.attrsOf valueType)
        (t.listOf valueType)
      ])
      // {
        description = "RON value";
      };
  in
    valueType;

  renderDocument = value:
    lib.concatMapStrings (extension: "#![enable(${extension})]\n") cfg.ronExtensions
    + ron.toRON value
    + "\n";
in {
  options.programs.rmpc = {
    settings = lib.mkOption {
      type = lib.types.attrsOf ronValue;
      default = {};
      example = lib.literalExpression ''
        {
          address = "127.0.0.1:6600";
          theme = "mytheme";
          album_art.method = rmpcLib.variant "Kitty";
          tabs = [
            (rmpcLib.tab "Queue" "Queue")
            (rmpcLib.tab "Search" "Search")
          ];
        }
      '';
      description = ''
        rmpc configuration, rendered to RON and written to
        {file}`$XDG_CONFIG_HOME/rmpc/config.ron`. RON constructs with no
        native Nix representation are built with the helpers provided
        through the `rmpcLib` module argument. Mutually exclusive with the
        plain `config` option.
      '';
    };

    themes = lib.mkOption {
      type = lib.types.attrsOf (lib.types.attrsOf ronValue);
      default = {};
      example = lib.literalExpression ''
        {
          mytheme = {
            background_color = "#1e1e2e";
            tab_bar.active_style = { fg = "black"; bg = "#cba6f7"; modifiers = "Bold"; };
          };
        }
      '';
      description = ''
        Themes in the same representation as {option}`programs.rmpc.settings`,
        each rendered to {file}`$XDG_CONFIG_HOME/rmpc/themes/<name>.ron`.
        Select one by setting `settings.theme` to its attribute name.
      '';
    };

    ronExtensions = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [
        "implicit_some"
        "unwrap_newtypes"
        "unwrap_variant_newtypes"
      ];
      description = ''
        RON extensions emitted as `#![enable(...)]` header lines in the
        generated files. The default matches rmpc's default config.
      '';
    };
  };

  config = lib.mkMerge [
    # Outside the mkIf so config modules can use rmpcLib unconditionally.
    {_module.args.rmpcLib = rmpcLib;}

    (lib.mkIf cfg.enable {
      programs.rmpc.config = lib.mkIf (cfg.settings != {}) (renderDocument cfg.settings);

      xdg.configFile =
        lib.mapAttrs' (
          name: theme:
            lib.nameValuePair "rmpc/themes/${name}.ron" {text = renderDocument theme;}
        )
        cfg.themes;
    })
  ];
}
