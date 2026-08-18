# Structure review

Date: 2026-08-17

The workspace layout matches the documented crate, CLI, Desktop, and text UI
boundaries. No module relocation was applied: the audit did not find a
low-risk move with a clear ownership benefit, and moving files would expand the
review surface without repairing a defect.
