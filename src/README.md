# Frontend structure

The frontend is a React + TypeScript app rendered by Tauri. Code is grouped by
responsibility so the app shell stays separate from reusable UI and domain logic.

```text
src/
├── main.tsx                  # React entry point and providers
├── app/
│   ├── App.tsx               # Application shell and composition root
│   └── App.css               # Global design tokens, shell, and component styles
├── api/
│   └── index.ts              # Typed Tauri command client and shared data types
├── components/
│   ├── common/               # Small reusable primitives (Icon, Dropdown, Resizer)
│   ├── feedback/             # App-wide feedback providers (toast, confirm)
│   ├── dialogs/              # Modal dialogs (settings, help, manage, updates)
│   ├── editor/               # Markdown editor, preview, and live-preview plugin
│   ├── notes/                # Note list, editor pane, and metadata controls
│   └── sidebar/              # Notebook tree, search, and filter controls
├── hooks/
│   └── useUpdater.ts         # Reusable stateful logic for app updates
└── lib/
    ├── filters.ts            # Note filter state and helpers
    ├── shortcuts.ts          # Keyboard shortcut definitions and dispatch
    ├── theme.ts              # Theme preference persistence and application
    └── tree.ts               # Notebook tree building and move calculations
```

## Conventions

- Components use `PascalCase.tsx`; hooks use `useSomething.ts`; helpers use
  `camelCase.ts`.
- Keep side effects and Tauri calls at the edges (`api/`, `hooks/`, dialogs, and
  the app shell). `lib/` stays framework-light and easy to test.
- Import direction is `app/` → `components/` / `hooks/` / `lib/` → `api/`.
  Shared code must not import from `app/`.
- Add new UI to the narrowest folder that owns it. Promote a component to
  `common/` only after it is used by more than one domain.
- Import a folder's public API through its `index.ts` (for example
  `import { api } from "../api"`). Import components from their concrete files
  to keep dependencies visible.
- Colocate tests and Storybook stories next to their component when added.
