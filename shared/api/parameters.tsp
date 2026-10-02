import "@typespec/http";

using Http;

namespace jjforge.Parameters;

model Org {
  @path org: jjforge.Name;
}

model Repo {
  @path repo: jjforge.Name;
}

model Principal {
  @format("uuid") @path principal: string;
}

model Rev {
  /** A change ID, a commit ID, a unique prefix of either, or a bookmark. Defaults to the trunk bookmark. */
  @query rev?: string;
}

model RevPath {
  @path rev: string;
}

model Path {
  /** A directory relative to the repository root. Defaults to the root. */
  @query path?: string;
}

model FilePath {
  /** A file relative to the repository root. */
  @query path: string;
}

model Cursor {
  /** The `next` value of the previous page. */
  @query cursor?: string;
}

model Limit {
  @minValue(1)
  @maxValue(100)
  @query
  limit?: integer = 50;
}

/** The parameters of every list: where to continue, and how many. */
alias Paging = {
  ...Parameters.Cursor;
  ...Parameters.Limit;
};

/** The repository an operation acts on. */
alias InRepo = {
  ...Parameters.Org;
  ...Parameters.Repo;
};
