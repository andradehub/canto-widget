using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
using System.Windows;
using System.Windows.Interop;
using System.Windows.Threading;

namespace CantoWidget;

// Windows 10's taskbar hosts this window as a real child. The task buttons
// receive a smaller rectangle, so the widget does not cover them or the tray.
internal sealed class TaskbarHost : IDisposable
{
    private readonly Window window;
    private readonly DispatcherTimer timer = new() { Interval = TimeSpan.FromSeconds(1) };
    private IntPtr taskbar;
    private IntPtr rebar;
    private IntPtr taskButtons;
    private Rect originalRebar;
    private Rect originalButtons;
    private IntPtr handle;
    public bool Attached { get; private set; }
    public string Status { get; private set; } = "Conectando à barra de tarefas…";
    public TaskbarHost(Window window) { this.window = window; timer.Tick += (_, _) => Layout(); }
    public void Start() { handle = new WindowInteropHelper(window).Handle; Layout(); timer.Start(); }
    public void Layout()
    {
        if (handle == IntPtr.Zero || !IsWindow(handle)) return;
        var shell = FindWindowW("Shell_TrayWnd", null);
        if (shell == IntPtr.Zero) { Status = "Aguardando a barra de tarefas"; ShowWindow(handle, 0); return; }
        if (shell != taskbar || GetParent(handle) != shell)
        {
            RestoreSpace();
            taskbar = shell;
            rebar = FindWindowExW(shell, IntPtr.Zero, "ReBarWindow32", null);
            taskButtons = rebar != IntPtr.Zero ? FindWindowExW(rebar, IntPtr.Zero, "MSTaskSwWClass", null) : IntPtr.Zero;
            if (rebar == IntPtr.Zero || taskButtons == IntPtr.Zero)
            {
                Attached = false; Status = "Esta barra de tarefas não oferece o encaixe do Windows 10"; ShowWindow(handle, 0); return;
            }
            originalRebar = RelativeRect(rebar);
            originalButtons = RelativeRect(taskButtons);
            long style = GetWindowLongPtrW(handle, -16).ToInt64() & 0xFFFFFFFFL;
            SetWindowLongPtrW(handle, -16, new IntPtr((style & ~0x80000000L) | 0x40000000L));
            SetParent(handle, shell);
            if (GetParent(handle) != shell)
            {
                SetWindowLongPtrW(handle, -16, new IntPtr(style));
                Attached = false; Status = "Não foi possível anexar à barra de tarefas";
                AppLog.Write("Taskbar SetParent: " + Marshal.GetLastWin32Error()); ShowWindow(handle, 0); return;
            }
            Attached = true;
        }
        GetClientRect(shell, out var client);
        if (client.Right < client.Bottom * 3)
        {
            Status = "Coloque a barra de tarefas na horizontal para mostrar o Canto"; ShowWindow(handle, 0); return;
        }
        var notification = FindWindowExW(shell, IntPtr.Zero, "TrayNotifyWnd", null);
        if (notification == IntPtr.Zero) { Status = "Aguardando a área do relógio"; return; }
        int right = RelativeRect(notification).Left;
        // News/weather sits immediately before the notification area on Windows 10.
        EnumChildWindows(shell, (child, _) =>
        {
            if (GetParent(child) == shell && child != handle && IsWindowVisible(child))
            {
                var name = new StringBuilder(128); GetClassNameW(child, name, name.Capacity);
                if (name.ToString().StartsWith("DynamicContent", StringComparison.Ordinal))
                {
                    var fixedItem = RelativeRect(child);
                    if (fixedItem.Right - fixedItem.Left > 0 && fixedItem.Left > client.Right / 2) right = Math.Min(right, fixedItem.Left);
                }
            }
            return true;
        }, IntPtr.Zero);
        double scale = GetDpiForWindow(shell) / 96.0;
        int width = (int)Math.Round(420 * scale);
        int left = right - width;
        var start = FindWindowExW(shell, IntPtr.Zero, "Start", null);
        int minimumLeft = start == IntPtr.Zero ? 48 : RelativeRect(start).Right;
        if (left < minimumLeft + 100 * scale) { Status = "Sem espaço livre na barra de tarefas"; ShowWindow(handle, 0); RestoreSpace(); return; }
        window.Width = width / scale;
        window.Height = client.Bottom / scale;
        int rebarWidth = Math.Max(1, left - originalRebar.Left);
        SetWindowPos(rebar, IntPtr.Zero, originalRebar.Left, originalRebar.Top, rebarWidth, originalRebar.Bottom - originalRebar.Top, 0x14);
        SetWindowPos(taskButtons, IntPtr.Zero, originalButtons.Left, originalButtons.Top, Math.Max(1, rebarWidth - originalButtons.Left), originalButtons.Bottom - originalButtons.Top, 0x14);
        SetWindowPos(handle, IntPtr.Zero, left, 0, width, client.Bottom, 0x70);
        Attached = true; Status = "Integrado à barra de tarefas";
    }
    private static Rect RelativeRect(IntPtr child)
    {
        GetWindowRect(child, out var rect);
        var parent = GetParent(child);
        var point = new NativePoint { X = rect.Left, Y = rect.Top }; ScreenToClient(parent, ref point);
        return new Rect { Left = point.X, Top = point.Y, Right = point.X + rect.Right - rect.Left, Bottom = point.Y + rect.Bottom - rect.Top };
    }
    private void RestoreSpace()
    {
        if (rebar != IntPtr.Zero && IsWindow(rebar)) SetWindowPos(rebar, IntPtr.Zero, originalRebar.Left, originalRebar.Top, originalRebar.Right - originalRebar.Left, originalRebar.Bottom - originalRebar.Top, 0x14);
        if (taskButtons != IntPtr.Zero && IsWindow(taskButtons)) SetWindowPos(taskButtons, IntPtr.Zero, originalButtons.Left, originalButtons.Top, originalButtons.Right - originalButtons.Left, originalButtons.Bottom - originalButtons.Top, 0x14);
    }
    public void Dispose() { timer.Stop(); RestoreSpace(); }
    [StructLayout(LayoutKind.Sequential)] private struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] private struct NativePoint { public int X, Y; }
    private delegate bool EnumProc(IntPtr window, IntPtr state);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern IntPtr FindWindowW(string className, string? name);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern IntPtr FindWindowExW(IntPtr parent, IntPtr after, string className, string? name);
    [DllImport("user32.dll", SetLastError = true)] private static extern IntPtr SetParent(IntPtr child, IntPtr parent);
    [DllImport("user32.dll")] private static extern IntPtr GetParent(IntPtr child);
    [DllImport("user32.dll")] private static extern bool GetWindowRect(IntPtr window, out Rect rect);
    [DllImport("user32.dll")] private static extern bool GetClientRect(IntPtr window, out Rect rect);
    [DllImport("user32.dll")] private static extern bool ScreenToClient(IntPtr window, ref NativePoint point);
    [DllImport("user32.dll")] private static extern bool SetWindowPos(IntPtr window, IntPtr after, int x, int y, int width, int height, uint flags);
    [DllImport("user32.dll")] private static extern bool IsWindow(IntPtr window);
    [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] private static extern bool ShowWindow(IntPtr window, int command);
    [DllImport("user32.dll")] private static extern uint GetDpiForWindow(IntPtr window);
    [DllImport("user32.dll")] private static extern bool EnumChildWindows(IntPtr window, EnumProc callback, IntPtr state);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern int GetClassNameW(IntPtr window, StringBuilder buffer, int length);
    [DllImport("user32.dll")] private static extern IntPtr GetWindowLongPtrW(IntPtr window, int index);
    [DllImport("user32.dll")] private static extern IntPtr SetWindowLongPtrW(IntPtr window, int index, IntPtr value);
}
