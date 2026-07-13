# Serializes Nix values to RON. Nix has no syntax for RON enum variants
# (Auto, as opposed to the string "Auto") or named structs (Split(...));
# those are written as `__type`-tagged attrsets built with the constructors
# below, the same convention used by Home Manager's Cosmic modules. `null`
# renders as None: rmpc enables the implicit_some extension, so Some() is
# never needed and optionals only ever appear as a literal None.
{lib}: let
  inherit (builtins) head match stringLength toJSON typeOf;
  inherit (lib) all boolToString concatMapStringsSep concatStringsSep hasInfix mapAttrsToList;
  inherit (lib.strings) floatToString replicate;

  indent = level: replicate level "    ";

  # floatToString pads to six decimals (1.5 -> "1.500000"); trim the excess,
  # but keep one decimal so the value stays a float in RON.
  renderFloat = f: let
    s = floatToString f;
  in
    if hasInfix "." s
    then head (match "(-?[0-9]+[.][0-9]*[1-9]|-?[0-9]+[.]0)0*" s)
    else s;

  # One line when everything is short and flat, otherwise one element per line.
  layout = level: open: close: elements: let
    oneLine = "${open}${concatStringsSep ", " elements}${close}";
    fitsOneLine = all (e: !hasInfix "\n" e) elements && stringLength oneLine <= 60;
  in
    if elements == []
    then "${open}${close}"
    else if fitsOneLine
    then oneLine
    else "${open}\n${concatMapStringsSep "\n" (e: "${indent (level + 1)}${e},") elements}\n${indent level}${close}";

  renderFields = level: open: close: fields:
    layout level open close (mapAttrsToList (name: value: "${name}: ${render (level + 1) value}") fields);

  renderTagged = level: value:
    if value.__type == "raw"
    then value.value
    else if value.__type == "enum"
    then
      if value ? value
      then layout level "${value.variant}(" ")" (map (render (level + 1)) value.value)
      else value.variant
    else if value.__type == "namedStruct"
    then renderFields level "${value.name}(" ")" value.value
    else throw "ron.nix: unknown tagged value type `${toString value.__type}`";

  render = level: value:
    {
      bool = boolToString value;
      int = toString value;
      float = renderFloat value;
      string = toJSON value; # JSON string escaping is valid RON
      path = toJSON (toString value);
      null = "None";
      list = layout level "[" "]" (map (render (level + 1)) value);
      set =
        if value ? __type
        then renderTagged level value
        else renderFields level "(" ")" value;
    }
    .${
      typeOf value
    } or (throw "ron.nix: cannot serialize a ${typeOf value} to RON");
in {
  # variant "Auto" -> Auto
  variant = name: {
    __type = "enum";
    variant = name;
  };

  # enum "Text" ["-"] -> Text("-"); arguments are serialized, so variants nest
  enum = name: args: {
    __type = "enum";
    variant = name;
    value = args;
  };

  # struct "Split" {size = "50%";} -> Split(size: "50%")
  struct = name: fields: {
    __type = "namedStruct";
    name = name;
    value = fields;
  };

  # emitted into the output verbatim
  raw = value: {
    __type = "raw";
    inherit value;
  };

  toRON = render 0;
}
