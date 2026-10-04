import "@typespec/openapi";

using OpenAPI;

namespace jjforge;

model Echo {
  message: string;
}

@pattern("^[a-z0-9][a-z0-9-]{0,38}$")
scalar Name extends string;

model Principal {
  @format("uuid") id: string;
  kind: "human" | "agent" | "component" | "workload";
  name: string;
}

model Session {
  @format("uuid") id: string;
  client: string;
  createdAt: utcDateTime;
  lastUsedAt: utcDateTime;
  current: boolean;
}

model AuthorizationServerMetadata {
  issuer: url;
  authorization_endpoint: url;
  token_endpoint: url;
  device_authorization_endpoint: url;
  revocation_endpoint: url;
  keys_uri: url;
  scopes_supported?: string[];
}

model DeviceAuthorizationRequest {
  client_id: string;
  scope?: string;
}

model DeviceAuthorization {
  device_code: string;
  user_code: string;
  verification_uri: url;
  verification_uri_complete?: url;
  expires_in: integer;
  interval: integer;
}

model TokenRequest {
  grant_type:
    | "authorization_code"
    | "refresh_token"
    | "urn:ietf:params:oauth:grant-type:device_code";
  client_id: string;
  code?: string;
  redirect_uri?: url;
  code_verifier?: string;
  device_code?: string;
  refresh_token?: string;
  scope?: string;
}

model Token {
  access_token: string;
  token_type: "Bearer";
  expires_in: integer;
  refresh_token?: string;
  scope?: string;
}

model RevocationRequest {
  token: string;
  token_type_hint?: "access_token" | "refresh_token";
}

model KeySet {
  keys: {
    id: string;
    algorithm: "ed25519";

    /** Hex-encoded. */
    publicKey: string;

    expiresAt?: utcDateTime;
  }[];
}

model Org {
  @format("uuid") id: string;
  name: Name;
  role: Role;
}

model OrgCreate {
  name: Name;
}

enum Role {
  owner,
  member,
}

model IdentityProvider {
  issuer: url;
  clientId: string;
  @extension("writeOnly", true) clientSecret?: string;
}

model Member {
  principal: Principal;
  role: Role;
}

model MemberUpdate {
  role: Role;
}

model AccessRecord {
  @format("uuid") id: string;
  time: utcDateTime;
  principal: Principal;
  action: string;
  resource: string;
  decision: "allow" | "deny";

  /** The policy that decided. */
  policy: string;
}

model Repo {
  @format("uuid") id: string;
  org: Name;
  name: Name;

  /** The sync address `jf` clones from: `https://{host}/sync/v1/{orgId}/{repoId}`. */
  cloneUrl: url;

  createdAt: utcDateTime;
}

model RepoCreate {
  name: Name;
}

model Activity {
  id: string;
  time: utcDateTime;
  principal: Principal;
  kind: "push" | "bookmark_moved";
  bookmark?: string;

  /** The commit ID before. */
  from?: string;

  /** The commit ID after. */
  to?: string;
}

model Signature {
  name: string;
  email: string;
  time: utcDateTime;
}

model ChangeSummary {
  changeId: string;
  commitId: string;
  description: string;
  author: Signature;

  /** Parent commit IDs. */
  parents: string[];

  conflicted: boolean;
  bookmarks: string[];
}

model Change {
  changeId: string;
  commitId: string;
  description: string;
  author: Signature;
  committer: Signature;

  /** Parent commit IDs. */
  parents: string[];

  conflicted: boolean;
  bookmarks: string[];
  files: ChangedFile[];
}

model ChangedFile {
  path: string;

  /** Set for a rename. */
  oldPath?: string;

  status: "added" | "modified" | "deleted" | "renamed";
  conflicted: boolean;
}

model Version {
  commitId: string;
  time: utcDateTime;

  /** The operation that wrote this version. */
  operationId: string;
}

model Operation {
  id: string;
  time: utcDateTime;
  principal?: Principal;
  description: string;
  parents: string[];
}

model Bookmark {
  name: string;

  /** Change IDs. More than one means the bookmark is conflicted. */
  targets: string[];
}

model TreeEntry {
  name: string;
  path: string;
  kind: "file" | "directory" | "symlink" | "submodule";
  size?: int64;
}

model ConflictedFile {
  path: string;
  sides: {
    kind: "add" | "remove";
    binary: boolean;

    /** Absent for binary content. */
    content?: string;
  }[];
}

model FileDiff {
  path: string;

  /** Set for a rename. */
  oldPath?: string;

  status: "added" | "modified" | "deleted" | "renamed";

  /** True when the content isn't shown as text. */
  binary: boolean;

  hunks: Hunk[];
}

model Hunk {
  oldStart: integer;
  oldLines: integer;
  newStart: integer;
  newLines: integer;
  lines: {
    kind: "context" | "added" | "removed";
    text: string;
  }[];
}
/** One page of a list. `next` is set while more pages exist. */
model Page<T> {
  items: T[];
  next?: string;
}

model SessionPage is Page<Session>;
model OrgPage is Page<Org>;
model MemberPage is Page<Member>;
model AccessRecordPage is Page<AccessRecord>;
model RepoPage is Page<Repo>;
model ChangePage is Page<ChangeSummary>;
model VersionPage is Page<Version>;
model OperationPage is Page<Operation>;
model BookmarkPage is Page<Bookmark>;
model TreeEntryPage is Page<TreeEntry>;
model FileDiffPage is Page<FileDiff>;
