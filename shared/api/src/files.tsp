import "@typespec/http";
import "@typespec/openapi";

using Http;
using OpenAPI;

namespace jjforge;

// OpenAPI 3.1 emits `bytes` as contentMediaType, which the generators
// map to an untyped body. `format: binary` keeps them on byte arrays.
@format("binary")
scalar Binary extends string;

model TextFile {
  @header contentType: "text/plain";
  @body body: string;
}

model ConflictedFileBody {
  @header contentType: "application/json";
  @body body: ConflictedFile;
}

model BinaryFile {
  @header contentType: "application/octet-stream";
  @body body: Binary;
}

/** The requested range. */
model FileRange {
  @statusCode status: 206;
  @header contentType: "application/octet-stream";
  @body body: Binary;
}

@tag("files")
@summary("List a directory at a revision")
@returnsDoc("One page of entries, directories first, then by name.")
@get
@route("/api/v1/orgs/{org}/repos/{repo}/tree")
op listTree(
  ...Parameters.InRepo,
  ...Parameters.Rev,
  ...Parameters.Path,
  ...Parameters.Paging,
): TreeEntryPage | Unauthenticated | NotFound | Ambiguous;

/**
 * Text answers `text/plain`, and binary content answers
 * `application/octet-stream`. A `Range` header reads part of the file.
 * A conflicted file answers its sides as JSON.
 */
@tag("files")
@summary("Read a file at a revision")
@returnsDoc("The whole file.")
@get
@route("/api/v1/orgs/{org}/repos/{repo}/contents")
op readFile(
  ...Parameters.InRepo,
  ...Parameters.Rev,
  ...Parameters.FilePath,
  @header("Range") range?: string,
):
  | TextFile
  | BinaryFile
  | ConflictedFileBody
  | FileRange
  | Unauthenticated
  | NotFound
  | Ambiguous;

/** Pass `rev` for a change against its parents, or `from` and `to` for two revisions. */
@tag("files")
@summary("Compare a change with its parents, or two revisions")
@returnsDoc("One page of file diffs.")
@get
@route("/api/v1/orgs/{org}/repos/{repo}/diff")
op getDiff(
  ...Parameters.InRepo,
  @query rev?: string,
  @query from?: string,
  @query to?: string,
  ...Parameters.Paging,
): FileDiffPage | Invalid | Unauthenticated | NotFound | Ambiguous;
