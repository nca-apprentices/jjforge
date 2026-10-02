import "@typespec/http";

using Http;

namespace jjforge;

/** RFC 9457 problem details with a stable `code`. */
model Problem {
  code:
    | "invalid"
    | "unauthenticated"
    | "forbidden"
    | "not_found"
    | "rev_not_found"
    | "path_not_found"
    | "rev_ambiguous"
    | "name_taken"
    | "precondition_failed";
  title: string;
  status: integer;
  detail?: string;

  /** For `rev_ambiguous`, every change the revision matches. */
  candidates?: ChangeSummary[];
}

model OAuthError {
  error: string;
  error_description?: string;
}

// Each failure answers a Problem as application/problem+json, so an
// operation lists only the statuses it can answer, such as
// `Repo | Unauthenticated | NotFound`.
@error
model ProblemResponse<Status extends int32> {
  @statusCode status: Status;
  @header contentType: "application/problem+json";
  @body body: Problem;
}

/** The request is malformed. Code `invalid`. */
model Invalid is ProblemResponse<400>;

/** No valid token. Code `unauthenticated`. */
model Unauthenticated is ProblemResponse<401>;

/** The token doesn't allow this. Code `forbidden`. */
model Forbidden is ProblemResponse<403>;

/**
 * Not found, or not visible to the principal. Code `not_found`,
 * `rev_not_found`, or `path_not_found`.
 */
model NotFound is ProblemResponse<404>;

/** Conflicts with the current state. Code `name_taken` or `precondition_failed`. */
model Conflict is ProblemResponse<409>;

/** The revision matches more than one change. Code `rev_ambiguous`, with `candidates`. */
model Ambiguous is ProblemResponse<422>;

/** An OAuth error (RFC 6749, section 5.2). */
@error
model OAuthFailure {
  @statusCode status: 400;
  @body body: OAuthError;
}
