# mini-hypervisor → MinIOS early-boot integration

This integration proves one narrow executable contract between the imported `mini-hypervisor` and `minios-x86` projects without modifying either imported subtree.

## Contract

1. Build the real `projects/minios-x86/kernel.bin` ELF32 artifact.
2. Parse and load its ELF32 `PT_LOAD` segments into KVM guest RAM.
3. Install a minimal valid Multiboot v1 memory-info structure.
4. Start vCPU0 through the public `mini-hypervisor` real-mode API at a small guest-owned bridge.
5. The bridge installs a flat 32-bit GDT, enables CR0.PE, sets `EAX=0x2BADB002` and `EBX` to the Multiboot-info address, then jumps to the ELF entry point.
6. Capture MinIOS console output through the hypervisor's existing port-`0xE9` debug device.
7. Require the exact early kernel banner `Booting Advanced OS...\n`.
8. Require the next unsupported hardware boundary to be MinIOS's first PIC-remap write: `OUT 0x20`, one byte, one element.

The last assertion is deliberate. `mini-hypervisor` does not currently emulate MinIOS's legacy PIC/PIT/keyboard/ATA platform. Reaching the exact PIC boundary after the exact MinIOS banner proves the ELF loader, real→protected-mode bridge, Multiboot register contract, kernel entry, stack setup, C call boundary, VGA initialization, debug-port execution, and initial kernel control flow actually executed under KVM.

## Non-claims

This is **not** a full MinIOS boot, shell, device, interrupt, filesystem, networking, or performance claim. The verified edge ends at the first intentionally unsupported legacy-device I/O access. Extending the edge past that point requires an explicit device/platform contract rather than treating an unsupported port as success.
