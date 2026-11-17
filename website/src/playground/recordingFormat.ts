/**
 * What the recorder (../../recorder/record.ts) writes and the playground replays: what the
 * built docattest printed for every command of every stop, in every setting of the stop's
 * switches. Both sides use these types, so they cannot drift apart.
 */
export interface Recordings {
  /** The invented project every run starts from, as path to text. */
  readonly project: Readonly<Record<string, string>>;
  readonly stops: Readonly<Record<string, RecordedStop>>;
}

export interface RecordedStop {
  /** Edits already made when the visitor arrives. */
  readonly given: readonly Edit[];
  readonly switches: readonly Switch[];
  /** Keyed by the index of the chosen option of each switch, joined: "01" is the first
   *  switch as it was and the second one changed. */
  readonly states: Readonly<Record<string, RecordedState>>;
}

/** One text replacement in one project file. */
export interface Edit {
  readonly file: string;
  readonly find: string;
  readonly replace: string;
}

/** A spot in a file the visitor can change. The first option is what the file says to begin
 *  with, and occurs in it exactly once; every option is one line of text. */
export interface Switch {
  readonly file: string;
  readonly options: readonly SwitchOption[];
}

export interface SwitchOption {
  readonly text: string;
  /** What the page shows for the spot when the text is empty. */
  readonly shown?: string;
}

export interface RecordedState {
  /** By command id. */
  readonly outputs: Readonly<Record<string, CommandOutput>>;
  /** Files the commands wrote, as path to text. */
  readonly written: Readonly<Record<string, string>>;
}

export interface CommandOutput {
  /** The command as a visitor would type it. */
  readonly command: string;
  /** The event an agent sends a hook, when the command reads one. */
  readonly stdin?: string;
  readonly stdout: string;
  readonly stderr: string;
  readonly exitCode: number;
}
