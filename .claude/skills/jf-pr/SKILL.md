---
name: jf-pr
description: Open a pull request in this repository, alone or stacked on another branch. Use when asked to open, create, or stack a PR.
---

# Open a pull request

1. **Base.** `main`, or the branch below this one in a stack. Push with
   `git push -u origin <branch>`.
2. **Title.** The commit subject. [The workflow](../../../shared/docs/workflow.md#specifying-and-building)
   gives the title of a spec PR and a build PR.
3. **Body.** Markdown, one paragraph per line, never wrapped by hand:
   - `## Summary`: what changes. One or two sentences.
   - `## Motivation`: why the change matters. Link the ADR or the issue behind it.
   - `## Notes`: each check you ran and what it showed, and where the change
     departs from its issue.
   - Last, one per line, after one blank line:
     - `Closes #<n>` for the requirement, task, or bug the PR finishes,
       without a colon, the form the Spec workflow reads.
     - `Part of #<epic>` for a spec PR, which finishes no issue.
     - `Ref #<n>` for each issue it advances without finishing.
4. **Stack.** A stacked PR's notes start with `Stacked on #<PR below>`.
   Before the PR below merges, point this one at `main` with
   `gh pr edit <n> --base main`, because GitHub closes a PR whose base
   branch is deleted. After the squash merge, rebase with
   `git rebase --onto main <old base commit>` and push.
5. **Open.** `gh pr create --base <base> --title <title> --body-file <file>`.
   Print the URL.
