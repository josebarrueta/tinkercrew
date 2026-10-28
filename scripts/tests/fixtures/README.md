# Historical prepared-profile fixture

`companion-pre-usb-{server,core}.py` are the repository's own MIT-licensed files at commit `2fa3d38`, before temporary USB gameplay and the inbound Bridge relay. They freeze the bytes shared by the exact historical source releases `63e90d…` and `d3c762…`.

Only the narrow never-started profile-correction tests use it. They must continue to verify those original receipts rather than silently treating each new gameplay runtime as a new correction target. Current initial provisioning and runtime-deployment tests still use the current compiled package. This fixture is not shipped as a replacement Companion or restoration backup.
