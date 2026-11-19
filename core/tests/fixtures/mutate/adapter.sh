# A fake adapter for the core's tests of `mutate` (src/mutate/tests.rs, tests/mutate_signal.rs):
# it answers what the core asks of a project whose "constants" are the files src/k.txt and
# src/n.txt. It gives an edit of each, for K and N, and none for F; while src/k.txt is changed,
# the project cannot be evaluated at all. The fact on line 1 gives what src/n.txt holds, the
# one on line 3 reads N but gives 7 whatever it holds, the one on line 5 reads only the
# function F, and the one on line 7 only K.
while read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/^{"id":\([0-9]*\).*/\1/p')
  method=$(printf '%s' "$line" | sed -n 's/.*"method":"\([a-zA-Z]*\)".*/\1/p')
  case "$method" in
    hello) result='{"protocol":1,"adapter":{"name":"fake","version":"0"},"blockLanguages":[],"sourceFiles":{"include":[],"exclude":[]},"ignoredNames":[],"capabilities":["mutants"]}' ;;
    open) root=$(printf '%s' "$line" | sed -n 's/.*"root":"\([^"]*\)".*/\1/p'); result='{}' ;;
    symbols) result='{"docs/a.md:1:fact:0":[{"name":"K"},{"name":"N"}],"docs/a.md:3:fact:1":[{"name":"N"}],"docs/a.md:5:fact:2":[{"name":"F"}],"docs/a.md:7:fact:3":[{"name":"K"}]}' ;;
    evaluate)
      # A run that must be stopped while it works: every evaluation in a copy takes long.
      case "$root" in *.docattest-mutate-*) [ -n "$FAKE_ADAPTER_SLOW_IN_COPIES" ] && sleep 30 ;; esac
      if [ "$(cat "$root/src/k.txt")" = broken ]; then
        printf '{"id":%s,"error":{"kind":"user","message":"docattest: the project does not load"}}\n' "$id"
        continue
      fi
      n=$(tr -d '\n' < "$root/src/n.txt")
      value() { printf '"docs/a.md:%s":{"ok":true,"value":{"text":"%s","description":"x"}}' "$1" "$2"; }
      result="{$(value 1:fact:0 "$n"),$(value 3:fact:1 7),$(value 5:fact:2 2),$(value 7:fact:3 5)}" ;;
    mutants) result='[{"edits":[{"path":"src/k.txt","start":0,"end":1,"replacement":"broken","description":"1 → broken"}]},{"edits":[{"path":"src/n.txt","start":0,"end":1,"replacement":"4","description":"3 → 4"}]},{"edits":[],"why":"function"}]' ;;
    *) result='{}' ;;
  esac
  printf '{"id":%s,"result":%s}\n' "$id" "$result"
done
