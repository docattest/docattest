use super::*;
use crate::agents::{DEFAULT, SHIPPED, read_agent};
use crate::git::tests::Temp;
use std::fs;
use std::path::PathBuf;

fn message<T: std::fmt::Debug>(result: Result<T>) -> String {
    match result {
        Err(Error::User(m)) => m,
        other => format!("{other:?}"),
    }
}

fn command(run: &str) -> impl Fn(&str) -> String + '_ {
    move |action| format!("{run} {action} --hook")
}

/// An agent nobody ships, whose entries are groups of handlers.
const GROUPED: &str = r#"{
      "name": "grouped",
      "projectRoot": "\"$ITS_ROOT\"",
      "settings": { "file": ".agent/settings.json", "hooks": "events", "group": "handlers", "handler": { "kind": "shell" }, "command": "line" },
      "hooks": {
        "impact": { "event": "AfterEdit", "matcher": "edit|write", "file": ["input", "path"], "reply": { "fixed": { "out": { "event": "AfterEdit" } }, "text": ["out", "context"] } },
        "check": { "event": "BeforeStop", "continuing": ["again"], "reply": { "fixed": { "verdict": "stay" }, "text": ["why"] } }
      }
    }"#;

/// Another, unlike it in every way a descriptor can say: its file, its events, entries
/// that are handlers themselves, the key of a command, a key the file must have, where
/// the event holds things and what the answer looks like.
pub(crate) const FLAT: &str = r#"{
      "name": "flat",
      "settings": { "file": "conf/agent.json", "hooks": "on", "command": "run", "handler": { "shell": true }, "defaults": { "version": 2 } },
      "hooks": {
        "impact": { "event": "edited", "matcher": "*.src", "file": ["path"], "reply": { "text": ["say", "to", "agent"] } },
        "check": { "event": "done", "continuing": ["retries"], "reply": { "fixed": { "again": true }, "text": ["because"] } }
      }
    }"#;

fn agent(descriptor: &str) -> Agent {
    read_agent(&parse_json(descriptor).unwrap(), "test").unwrap()
}

#[test]
fn writes_both_hooks_into_a_new_settings_file_in_the_shape_the_agent_reads() {
    let root = Temp::new("hooks");
    let (file, added) = install_hooks(&root.0, &agent(GROUPED), command("npx --no docattest")).unwrap();
    let events: Vec<&str> = added.iter().map(|a| a.event.as_str()).collect();
    assert_eq!(events, ["AfterEdit", "BeforeStop"]);
    assert_eq!(file, root.0.join(".agent/settings.json").display().to_string());
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        r#"{
  "events": {
    "AfterEdit": [
      {
        "matcher": "edit|write",
        "handlers": [
          {
            "kind": "shell",
            "line": "npx --no docattest impact --hook"
          }
        ]
      }
    ],
    "BeforeStop": [
      {
        "handlers": [
          {
            "kind": "shell",
            "line": "npx --no docattest check --hook"
          }
        ]
      }
    ]
  }
}
"#
    );
}

#[test]
fn writes_another_agents_hooks_in_that_agents_shape() {
    let root = Temp::new("hooks");
    root.write("conf/agent.json", r#"{"on":{"done":[{"run":"echo bye"}]},"mine":1}"#);
    let (file, added) = install_hooks(&root.0, &agent(FLAT), command("docattest")).unwrap();
    assert_eq!(file, root.0.join("conf/agent.json").display().to_string());
    assert_eq!(added.len(), 2);
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        r#"{
  "on": {
    "done": [
      {
        "run": "echo bye"
      },
      {
        "shell": true,
        "run": "docattest check --hook"
      }
    ],
    "edited": [
      {
        "matcher": "*.src",
        "shell": true,
        "run": "docattest impact --hook"
      }
    ]
  },
  "mine": 1,
  "version": 2
}
"#
    );
    assert!(install_hooks(&root.0, &agent(FLAT), command("docattest")).unwrap().1.is_empty());
    // A hook in a group is not one of this agent's, and a key the file has is kept.
    root.write(
        "conf/agent.json",
        r#"{"version":7,"on":{"done":[{"handlers":[{"run":"docattest check"}]}]}}"#,
    );
    let (_, added) = install_hooks(&root.0, &agent(FLAT), command("docattest")).unwrap();
    assert_eq!(added.len(), 2);
    assert!(fs::read_to_string(&file).unwrap().starts_with("{\n  \"version\": 7,\n"));
}

#[test]
fn keeps_what_is_there_and_never_adds_a_hook_twice() {
    let root = Temp::new("hooks");
    let grouped = agent(GROUPED);
    let settings = ".agent/settings.json";
    root.write(
            settings,
            r#"{"allow":["ls"],"events":{"AfterEdit":[{"matcher":"shell","handlers":[{"kind":"shell","line":"echo hi"}]}],"BeforeStop":[{"handlers":[{"kind":"shell","line":"docattest check"}]}]}}"#,
        );
    let (_, added) = install_hooks(&root.0, &grouped, command("py -m docattest")).unwrap();
    assert_eq!(added.iter().map(|h| h.event.as_str()).collect::<Vec<_>>(), vec!["AfterEdit"]);
    assert_eq!(added.first().map(|h| h.command.as_str()), Some("py -m docattest impact --hook"));
    let text = fs::read_to_string(root.0.join(settings)).unwrap();
    let after: Map<String, Value> = serde_json::from_str(&text).unwrap();
    assert_eq!(after["allow"].to_string(), r#"["ls"]"#);
    let hooks = after["events"].as_object().unwrap();
    let Some(Value::Array(post)) = hooks.get("AfterEdit") else { panic!("AfterEdit is a list") };
    assert_eq!(post.len(), 2);
    assert_eq!(post[0].to_string(), r#"{"matcher":"shell","handlers":[{"kind":"shell","line":"echo hi"}]}"#);
    assert_eq!(
        hooks["BeforeStop"].to_string(),
        r#"[{"handlers":[{"kind":"shell","line":"docattest check"}]}]"#
    );
    assert!(install_hooks(&root.0, &grouped, command("docattest")).unwrap().1.is_empty());
    assert_eq!(fs::read_to_string(root.0.join(settings)).unwrap(), text);
}

#[test]
fn refuses_a_settings_file_it_cannot_merge_into_leaving_it_alone() {
    let root = Temp::new("hooks");
    let grouped = agent(GROUPED);
    let settings = ".agent/settings.json";
    let install = || install_hooks(&root.0, &grouped, command("docattest"));
    let file = root.0.join(settings).display().to_string();
    root.write(settings, "{ not json");
    assert_eq!(
        message(install()),
        format!(
            "docattest: {file} is not valid JSON (key must be a string at line 1 column 3); fix it and run again"
        )
    );
    root.write(settings, r#"{"events":{"BeforeStop":{}}}"#);
    assert_eq!(
        message(install()),
        format!("docattest: {file}: \"events.BeforeStop\" is not a list; fix it and run again")
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), r#"{"events":{"BeforeStop":{}}}"#);
    root.write(settings, "[]");
    assert_eq!(message(install()), format!("docattest: {file} does not hold a JSON object"));
    root.write(settings, r#"{"events":[]}"#);
    assert_eq!(message(install()), format!("docattest: {file}: \"events\" is not an object"));
}

#[test]
fn a_hook_says_which_agent_runs_it_unless_it_is_the_one_meant_by_default() {
    let root = PathBuf::from("/work/p");
    let load = |given: Option<&str>| load_agent(given, |file| PathBuf::from(file)).unwrap();
    assert_eq!(agent_argument(&load(None), &root), "");
    assert_eq!(agent_argument(&load(Some(DEFAULT)), &root), "");
    for (name, _) in SHIPPED.iter().filter(|(name, _)| *name != DEFAULT) {
        assert_eq!(agent_argument(&load(Some(name)), &root), format!(" --agent {name}"));
    }
    let mine =
        |file: &str, agent: Agent| Loaded { agent, file: Some(PathBuf::from(file)), is_default: false };
    assert_eq!(
        agent_argument(&mine("/work/p/my agent.json", agent(FLAT)), &root),
        " --agent ./'my agent.json'"
    );
    assert_eq!(agent_argument(&mine("/work/q/a.json", agent(FLAT)), &root), " --agent /work/q/a.json");
    assert_eq!(
        agent_argument(&mine("/work/p/a/b.json", agent(GROUPED)), &root),
        " --agent \"$ITS_ROOT\"/a/b.json"
    );
}

#[test]
fn prints_what_it_added_then_that_nothing_was_needed() {
    let root = Temp::new("hooks");
    root.write("flat.json", FLAT);
    let run = |args: &[&str]| {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let argv: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        let code = crate::cli::run(&argv, &mut Io { out: &mut out, err: &mut err, cwd: root.0.clone() });
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    };
    let file = root.0.join("conf/agent.json").display().to_string();
    let (code, out, _) = run(&["hooks", "install", "--agent", "./flat.json"]);
    assert_eq!(code, 0);
    assert_eq!(
        out,
        format!(
            "docattest hooks: added to {file}\n  edited (*.src): docattest impact --hook --agent ./flat.json\n  done: docattest check --hook --agent ./flat.json\n"
        )
    );
    assert_eq!(
        run(&["hooks", "install", "--agent", "flat.json"]),
        (0, format!("docattest hooks: already installed in {file}\n"), String::new())
    );
    let usage = "docattest: usage: docattest hooks install [--agent <name or file>] [--local] [--git]\n";
    assert_eq!(run(&["hooks"]), (1, String::new(), usage.into()));
    assert_eq!(run(&["hooks", "install", "x"]).0, 1);
    assert_eq!(
        run(&["hooks", "install", "--local", "--git"]),
        (1, String::new(), "docattest: --local cannot be used with --git\n".into())
    );
    assert_eq!(
        run(&["hooks", "install", "--local", "--agent", "./flat.json"]),
        (
            1,
            String::new(),
            "docattest: agent \"flat\" has no local settings file: its descriptor names none\n".into(),
        )
    );
    let (code, _, err) = run(&["hooks", "install", "--agent", "nobody"]);
    assert_eq!(code, 1);
    assert!(err.starts_with("docattest: unknown agent \"nobody\" (known: "), "{err}");
    assert!(err.ends_with("pass its descriptor file, as in --agent ./nobody.json\n"), "{err}");
    // With no agent named, the one meant by default is installed, whichever it is.
    let (code, out, _) = run(&["hooks", "install"]);
    let default = load_agent(None, |file| PathBuf::from(file)).unwrap().agent;
    let settings = root.0.join(default.settings.file).display().to_string();
    assert_eq!(
        (code, out.lines().next()),
        (0, Some(format!("docattest hooks: added to {settings}").as_str()))
    );
    if let Some(local_file) = default.settings.local_file {
        let local = root.0.join(local_file).display().to_string();
        let (code, out, _) = run(&["hooks", "install", "--local"]);
        assert_eq!(
            (code, out.lines().next()),
            (0, Some(format!("docattest hooks: added to {local}").as_str()))
        );
    }
}
