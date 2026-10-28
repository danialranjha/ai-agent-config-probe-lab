# Exact component evidence

11 selected fixtures × 3 methods = 33 rows, plus two removal traces. These are designed cases, not sampled attack attempts. No model, coding CLI, Asyar application or MCP server ran.

| Fixture | Registration during check: file changed | Status only: file changed | Explicit registration: file changed |
|---|---|---|---|
| missing_file | true | false | true |
| empty_object | true | false | true |
| unrelated_settings | true | false | true |
| removed_registration | true | false | true |
| already_matching | false | false | false |
| changed_command | true | false | true |
| extra_arguments | false | false | false |
| malformed_json | true | false | true |
| non_object_json | true | false | true |
| malformed_servers | true | false | true |
| non_utf8 | true | false | true |

Changed means file existence or exact bytes changed, independently read after the action. A Boolean returned by the helper is not the oracle. Missing files and missing parent directories remain absent under Status only.

## Main paired trace

Identical initial bytes in each branch:

```json
{"mcpServers":{},"theme":"light"}
```

**registration_during_check**, final bytes:

```json
{
  "mcpServers": {
    "asyar": {
      "args": [
        "mcp-server"
      ],
      "command": "INERT_ASYAR_COMMAND"
    }
  },
  "theme": "light"
}
```

**status_only**, final bytes:

```json
{"mcpServers":{},"theme":"light"}

```

Both paths observe the file. Registration during check additionally calls the unchanged upstream helper. Status only performs reads and reports the missing registration. Explicit registration is a separate harness action calling that same helper; it is a positive control, not a tested consent UI.

## Controls and limits

The already-matching fixture stays byte-identical in all three methods. Existing unrelated valid keys remain in the registration result. After explicit registration and removal, registration during check reinstates the entry; status-only inspection preserves the removed state.

The original helper accepts extra arguments as long as one is mcp-server, so extra_arguments receives no write although the strict observer reports different_registration. Matching describes only the command and argument fields, not permission or readiness. The helper replaces malformed/non-object/non-UTF8 documents with new configuration; explicit registration retains that behavior. Status only preserves the bytes and reports invalid data.

Normal suite: 11 passed, zero failed/ignored. Functional faults: status_registers: 4 assertion failures, explicit_disabled: 3 assertion failures. No compilation error is counted as a detected fault.
