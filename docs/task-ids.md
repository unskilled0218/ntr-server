# Task IDs

- [File backend](#file-backend)
- [Home Assistant backend](#home-assistant-backend)

## File backend

IDs remain stable within one running process. Deletion does not renumber other tasks; a removed ID returns 404. Restart or bulk replacement can reassign IDs, so clients must revalidate before using them.

## Home Assistant backend

The private sidecar persists IDs by HA instance/list/item UID. Duplicate UIDs in different lists remain distinct. IDs are globally monotonic within JavaScript's safe integer range and deleted IDs are not reused. A verified move explicitly transfers the source ID; retained partial copies otherwise have separate IDs.

Recovery confirms a monotonic operation ID, preventing an old confirmation from clearing a newer journal. Back up before migration; corrupt maps or another HA source fail closed. See [HA safety](home-assistant.md#safety-and-compatibility).
