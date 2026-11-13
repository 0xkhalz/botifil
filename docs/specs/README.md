# Specifications Workflow

Feature specifications and task decompositions for `botifil`.

Specifications in `docs/specs/` are optional and used for large multi-PR features; single-PR tasks use a GitHub Issue directly.

## Directory Convention

Each feature or slice lives in an isolated folder:

```text
docs/specs/NNN-slug/
├── spec.md   # Requirements, scope, and interfaces
├── plan.md   # Architectural design and execution plan
└── tasks.md  # Granular tasks for implementation
```

Templates are located in `.agents/skills/write-spec/assets/`.

## Lifecycle

1. `draft`: Author spec, plan, and task decomposition.
2. `approved`: Maintainer reviews and merges the specification PR into `main`.
3. `implemented`: Slices are delivered through worktree PRs and verified.

## Task Sizing Rule

Every task in `tasks.md` maps to one `task.yml` issue sized at <= 300 LoC.
