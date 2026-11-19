# The cases as data: a directory another repository can build its own tests
# on. It holds every case file, at the path it has under the cases'
# directory, and `cases.json`, which lists them all. The manifest is made
# here from the files, so a case is added by adding its file and nothing
# else.
#
# `cases.json` is `{ "cases": [ <case>… ] }`, in order of group, then name.
# Every path is relative to the directory. A case is one of three kinds:
#
#   { "kind": "valid", "group": "tree", "name": "a-pile-directory",
#     "type": "tree", "path": "tree/a-pile-directory.diagram" }
#     A diagram that must read and draw. Its group is its diagram type.
#
#   { "kind": "invalid", "group": "invalid", "name": "cell-taken",
#     "path": "invalid/cell-taken.diagram",
#     "expect": { "code": "cell-taken", "line": 8, "column": 5 } }
#     A file that must be refused, with the finding its first line says:
#     `// expect: <code> at <line>:<column>`.
#
#   { "kind": "diff", "group": "diff", "name": "a-crate-is-added",
#     "before": "diff/a-crate-is-added/before.diagram",
#     "after": "diff/a-crate-is-added/after.diagram" }
#     Two versions of one diagram, to compare.
#
# A group is a directory of cases: `invalid`, `diff`, and one for each
# diagram type. Anything that is not a case where one belongs stops the
# build, and says which path: the manifest never leaves a file out.
{
  lib,
  runCommand,
  jq,
  # The directory the cases are read from.
  caseFiles ? ./crates/pipeline/cases,
}:
let
  # A pile's own files are not cases.
  notCases = [ ".pile" ];

  malformed = path: why: throw "cases: ${path} ${why}";

  # What a directory holds, as { name, type } in order of name.
  entries =
    dir:
    lib.mapAttrsToList (name: type: { inherit name type; }) (
      removeAttrs (builtins.readDir (caseFiles + "/${dir}")) notCases
    );

  groups = map (
    entry:
    if entry.type == "directory" then
      entry.name
    else
      malformed entry.name "is in no group: a case is a file in a directory of cases"
  ) (entries "");

  # The names of the diagram files a directory holds, which must be all it holds.
  diagrams =
    dir:
    map (
      entry:
      if entry.type == "regular" && lib.hasSuffix ".diagram" entry.name then
        lib.removeSuffix ".diagram" entry.name
      else
        malformed "${dir}/${entry.name}" "is not a .diagram file"
    ) (entries dir);

  valid = group: name: {
    kind = "valid";
    inherit group name;
    type = group;
    path = "${group}/${name}.diagram";
  };

  invalid =
    name:
    let
      path = "invalid/${name}.diagram";
      first = lib.head (lib.splitString "\n" (builtins.readFile (caseFiles + "/${path}")));
      expect = builtins.match "// expect: ([a-z0-9-]+) at ([1-9][0-9]*):([1-9][0-9]*)" first;
    in
    {
      kind = "invalid";
      group = "invalid";
      inherit name path;
      expect =
        if expect == null then
          malformed path "does not open with // expect: <code> at <line>:<column>"
        else
          {
            code = lib.elemAt expect 0;
            line = lib.toInt (lib.elemAt expect 1);
            column = lib.toInt (lib.elemAt expect 2);
          };
    };

  diff =
    entry:
    let
      dir = "diff/${entry.name}";
      sides = if entry.type == "directory" then diagrams dir else malformed dir "is not a directory";
    in
    if
      sides == [
        "after"
        "before"
      ]
    then
      {
        kind = "diff";
        group = "diff";
        inherit (entry) name;
        before = "${dir}/before.diagram";
        after = "${dir}/after.diagram";
      }
    else
      malformed dir "does not hold before.diagram and after.diagram, and nothing else";

  cases = lib.concatMap (
    group:
    if group == "invalid" then
      map invalid (diagrams group)
    else if group == "diff" then
      map diff (entries group)
    else
      map (valid group) (diagrams group)
  ) groups;

  files = builtins.path {
    name = "case-files";
    path = caseFiles;
    filter = path: _: !(lib.elem (baseNameOf path) notCases);
  };
in
runCommand "diagram-cases"
  {
    nativeBuildInputs = [ jq ];
    manifest = builtins.toJSON { inherit cases; };
    passAsFile = [ "manifest" ];
  }
  ''
    cp -r ${files} $out
    chmod u+w $out
    jq . "$manifestPath" > $out/cases.json
  ''
