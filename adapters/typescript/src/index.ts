// The public API: `createProject`, a thin client of the docattest core (it runs the core
// that the `docattest` launcher runs), and the shapes it returns.
export {
  CHANGE_TYPES,
  COMMANDS,
  CONFIG_KEYS,
  DEFAULT_SNAPSHOT_MAX_AGE_DAYS,
  DEFAULT_TIMEOUT_MS,
  KINDS,
  STATUSES,
} from './api/constants.js';
export type { ChangeType, Kind, Status } from './api/constants.js';
export type { Config, Member, SurfaceEntry } from './api/config.js';
export type {
  Change,
  Coverage,
  MemberReport,
  Reference,
  Report,
  Result,
  SnapshotStatus,
  SurfaceCoverage,
  SurfaceResolution,
} from './api/report.js';
export type { CheckOptions, Project, ProjectInput } from './api/project.js';
export type {
  AuditReport,
  AuditStatus,
  AuditTarget,
  InventoryEntry,
  Suggestion,
} from './api/audit.js';
export type { Impact, ImpactOptions, ImpactReference } from './api/impact.js';
export type {
  Attestation,
  CasesAttestation,
  CasesRow,
  ExampleAttestation,
  FactAttestation,
  FactTarget,
  TestAttestation,
} from './api/attestation.js';
export { InternalError, UserError } from './api/errors.js';
export { createProject } from './client/project.js';
