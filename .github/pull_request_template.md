## Motivation

<!-- Why is this change needed? Link the approved spec, pipeline item, or issue. -->

## Summary

<!-- Describe the behavior and architecture changes. Keep unrelated work out. -->

## Compatibility

- [ ] Tauri IPC signatures remain compatible, or the breaking change is explicitly approved.
- [ ] Web API contracts remain compatible, or the breaking change is explicitly approved.
- [ ] SQLite schema/migrations are unchanged or safely migrated and tested.
- [ ] No production promotion, approval token, secret, database, or build artifact is included.

## Documentation and handoff

- [ ] User-visible behavior is documented.
- [ ] `docs/version-pipeline.md` contains the work item.
- [ ] The relevant active epitaph is updated or a new handoff is added.

## Testing

<!-- Prefix every exact command with ✅, ⚠️, or ❌ and include limitations. -->

- ✅ `scripts/ai-harness.sh check`
- ✅ `pnpm run test:gate`

## Known limitations

<!-- Write "None" when there are no known limitations. -->
