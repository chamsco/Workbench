# Windows check for the Cauldron shell's window controls (UI1).
# Launches the built app, sizes its client area to the wireframe's 1240x702, moves the mouse
# (window messages, the real cursor is not touched) over each control and asks Windows what
# it would do there (WM_NCHITTEST). GPUI answers from the last mouse position, so each point
# gets a WM_MOUSEMOVE first. Exits 1 on any mismatch.
#
#   pwsh tools/winshell/hit-test.ps1 [-Exe target/release/backspace.exe]
#
# ponytail: Windows only and needs a desktop session; GPUI's test platform has no public
# window-control hit test, so this is the check until it does.
param([string]$Exe = "target/release/backspace.exe")

Add-Type @"
using System; using System.Runtime.InteropServices;
public class H {
 [StructLayout(LayoutKind.Sequential)] public struct R { public int L,T,Ri,B; }
 [StructLayout(LayoutKind.Sequential)] public struct P { public int X,Y; }
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out R r);
 [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out R r);
 [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref P p);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x,int y,int cx,int cy,uint f);
 [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
 [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
}
"@
[H]::SetProcessDPIAware() | Out-Null

$proc = Start-Process -FilePath $Exe -PassThru
try {
    $h = [IntPtr]::Zero
    for ($i = 0; $i -lt 100 -and $h -eq [IntPtr]::Zero; $i++) { Start-Sleep -Milliseconds 100; $proc.Refresh(); $h = $proc.MainWindowHandle }
    if ($h -eq [IntPtr]::Zero) { throw "no window" }
    $r = New-Object H+R; [H]::GetWindowRect($h, [ref]$r) | Out-Null
    $c = New-Object H+R; [H]::GetClientRect($h, [ref]$c) | Out-Null
    [H]::SetWindowPos($h, [IntPtr]::Zero, 100, 100, 1240 + ($r.Ri - $r.L) - $c.Ri, 702 + ($r.B - $r.T) - $c.B, 0x0040) | Out-Null
    Start-Sleep -Milliseconds 800

    $names = @{ 1 = 'client'; 2 = 'drag'; 8 = 'min'; 9 = 'max'; 20 = 'close'; 11 = 'right'; 15 = 'bottom'; 17 = 'bottomright' }
    # Points in client coordinates, from crates/chrome layout::solve for a 1240x702 Windows window.
    $cases = @(
        @('minimise', 1129, 27, 'min'), @('maximise', 1165, 27, 'max'), @('close', 1201, 27, 'close'),
        @('sidebar button', 29, 27, 'client'), @('terminal', 1059, 27, 'client'), @('split', 1089, 27, 'client'),
        @('empty top strip', 1000, 20, 'drag'), @('main body', 600, 300, 'client'),
        @('right edge', 1242, 300, 'right'), @('bottom edge', 600, 704, 'bottom'), @('bottom-right corner', 1243, 705, 'bottomright')
    )
    $fail = 0
    foreach ($t in $cases) {
        [H]::PostMessage($h, 0x200, [IntPtr]0, [IntPtr](($t[2] -shl 16) -bor $t[1])) | Out-Null
        Start-Sleep -Milliseconds 300
        $p = New-Object H+P; $p.X = $t[1]; $p.Y = $t[2]; [H]::ClientToScreen($h, [ref]$p) | Out-Null
        $got = $names[[int][H]::SendMessage($h, 0x84, [IntPtr]0, [IntPtr](($p.Y -shl 16) -bor ($p.X -band 0xffff)))]
        $ok = $got -eq $t[3]
        if (-not $ok) { $fail++ }
        "{0} {1,-20} want {2,-12} got {3}" -f ($(if ($ok) { 'ok  ' } else { 'FAIL' }), $t[0], $t[3], $got)
    }
    "$($cases.Count - $fail) passed, $fail failed"
    if ($fail) { exit 1 }
} finally {
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
}
