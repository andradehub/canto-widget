using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Windows;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;
using Forms = System.Windows.Forms;
using Drawing = System.Drawing;

namespace CantoWidget;

// Windows 10's taskbar hosts this window as a real child. The task buttons
// receive a smaller rectangle, so the widget does not cover them or the tray.
internal sealed class TaskbarHost : IDisposable
{
    private readonly Window window;
    private readonly FrameworkElement content;
    private HwndSource? source;
    private TaskbarSurface? surface;
    private readonly DispatcherTimer timer = new() { Interval = TimeSpan.FromSeconds(1) };
    private IntPtr taskbar;
    private IntPtr rebar;
    private IntPtr taskButtons;
    private Rect originalRebar;
    private Rect originalButtons;
    private IntPtr handle;
    public bool Attached { get; private set; }
    public string Status { get; private set; } = "Conectando à barra de tarefas…";
    public TaskbarHost(Window window, FrameworkElement content) { this.window = window; this.content = content; timer.Tick += (_, _) => Layout(); }
    public void Start() { window.Content = null; window.Hide(); Layout(); timer.Start(); }
    public void Layout()
    {
        var shell = FindWindowW("Shell_TrayWnd", null);
        if (shell == IntPtr.Zero) { Status = "Aguardando a barra de tarefas"; if (handle != IntPtr.Zero) ShowWindow(handle, 0); return; }
        if (shell != taskbar || handle == IntPtr.Zero || !IsWindow(handle) || GetParent(handle) != shell)
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
            surface?.Dispose();
            if (source != null) { source.RootVisual = null; source.Dispose(); }
            var parameters = new HwndSourceParameters("CantoTaskbar") { ParentWindow = shell, WindowStyle = 0x40000000 | 0x10000000 | 0x04000000, ExtendedWindowStyle = 0x08000000, Width = 420, Height = 40 };
            source = new HwndSource(parameters);
            source.CompositionTarget.BackgroundColor = Color.FromRgb(7, 34, 52);
            source.CompositionTarget.RenderMode = RenderMode.SoftwareOnly;
            content.Width = 420; content.Height = 40;
            source.RootVisual = content;
            source.AddHook((IntPtr hwnd, int message, IntPtr wParam, IntPtr lParam, ref bool handled) => { if (message == 0x0010) { window.Dispatcher.BeginInvoke(window.Close); handled = true; } return IntPtr.Zero; });
            surface = new TaskbarSurface(shell, content, () => window.Dispatcher.BeginInvoke(window.Close));
            surface.CreateControl(); handle = surface.Handle;
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
        content.Width = width / scale;
        content.Height = client.Bottom / scale;
        int rebarWidth = Math.Max(1, left - originalRebar.Left);
        SetWindowPos(rebar, IntPtr.Zero, originalRebar.Left, originalRebar.Top, rebarWidth, originalRebar.Bottom - originalRebar.Top, 0x14);
        SetWindowPos(taskButtons, IntPtr.Zero, originalButtons.Left, originalButtons.Top, Math.Max(1, rebarWidth - originalButtons.Left), originalButtons.Bottom - originalButtons.Top, 0x14);
        SetWindowPos(handle, IntPtr.Zero, left, 0, width, client.Bottom, 0x70);
        if (source != null) SetWindowPos(source.Handle, handle, left, 0, width, client.Bottom, 0x50);
        surface?.RefreshFrame(scale);
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
    public void Dispose() { timer.Stop(); RestoreSpace(); surface?.Dispose(); surface = null; if (source != null) { source.RootVisual = null; source.Dispose(); source = null; } }
    [StructLayout(LayoutKind.Sequential)] private struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] private struct NativePoint { public int X, Y; }
    private delegate bool EnumProc(IntPtr window, IntPtr state);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern IntPtr FindWindowW(string className, string? name);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern IntPtr FindWindowExW(IntPtr parent, IntPtr after, string className, string? name);
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
}

// GDI composition keeps the child visible inside Explorer's taskbar. WPF still
// owns the visual tree, layout, popup and button events; this native surface
// presents that visual and routes pointer events back to the same controls.
internal sealed class TaskbarSurface : Forms.Control
{
    private readonly IntPtr shell;
    private readonly FrameworkElement content;
    private readonly Action close;
    private readonly Forms.ToolTip tooltip = new();
    private Drawing.Bitmap? frame;
    private double scale = 1;
    private System.Windows.Controls.Button? hovered;
    private Brush? previousBackground;
    public TaskbarSurface(IntPtr shell, FrameworkElement content, Action close)
    {
        this.shell = shell; this.content = content; this.close = close;
        SetStyle(Forms.ControlStyles.UserPaint | Forms.ControlStyles.AllPaintingInWmPaint | Forms.ControlStyles.OptimizedDoubleBuffer, true);
        BackColor = Drawing.Color.FromArgb(7, 34, 52); AccessibleName = "Canto: música e bateria do mouse"; TabStop = false;
    }
    protected override Forms.CreateParams CreateParams
    {
        get { var value = base.CreateParams; value.Parent = shell; value.Style = (value.Style & unchecked((int)~0x80000000u)) | 0x40000000 | 0x10000000; value.ExStyle |= 0x08000000; return value; }
    }
    public void RefreshFrame(double scale)
    {
        this.scale = scale;
        content.UpdateLayout();
        int width = Math.Max(1, (int)Math.Round(content.ActualWidth * scale));
        int height = Math.Max(1, (int)Math.Round(content.ActualHeight * scale));
        var target = new RenderTargetBitmap(width, height, 96 * scale, 96 * scale, PixelFormats.Pbgra32);
        target.Render(content);
        var encoder = new PngBitmapEncoder(); encoder.Frames.Add(BitmapFrame.Create(target));
        using var bytes = new MemoryStream(); encoder.Save(bytes); bytes.Position = 0;
        using var decoded = new Drawing.Bitmap(bytes);
        var old = frame; frame = new Drawing.Bitmap(decoded); old?.Dispose();
        Invalidate(); Update();
    }
    protected override void OnPaint(Forms.PaintEventArgs e) { base.OnPaint(e); if (frame != null) e.Graphics.DrawImageUnscaled(frame, 0, 0); }
    private DependencyObject? Hit(int x, int y) => content.InputHitTest(new Point(x / scale, y / scale)) as DependencyObject;
    private static System.Windows.Controls.Button? ButtonAt(DependencyObject? item)
    {
        while (item != null) { if (item is System.Windows.Controls.Button button) return button; item = VisualTreeHelper.GetParent(item); } return null;
    }
    protected override void OnMouseUp(Forms.MouseEventArgs e)
    {
        base.OnMouseUp(e);
        if (e.Button == Forms.MouseButtons.Left && ButtonAt(Hit(e.X, e.Y)) is { IsEnabled: true } button) button.RaiseEvent(new RoutedEventArgs(System.Windows.Controls.Button.ClickEvent));
        RefreshFrame(scale);
    }
    protected override void OnMouseMove(Forms.MouseEventArgs e)
    {
        base.OnMouseMove(e);
        var hit = Hit(e.X, e.Y);
        var button = ButtonAt(hit);
        if (button != hovered)
        {
            if (hovered != null) hovered.Background = previousBackground;
            hovered = button;
            if (hovered != null) { previousBackground = hovered.Background; hovered.Background = new SolidColorBrush(Color.FromRgb(36, 66, 85)); }
            Cursor = button == null ? Forms.Cursors.Default : Forms.Cursors.Hand;
            RefreshFrame(scale);
        }
        string tip = "";
        for (var item = hit; item != null; item = VisualTreeHelper.GetParent(item)) if (item is FrameworkElement { ToolTip: not null } element) { tip = element.ToolTip.ToString() ?? ""; break; }
        if (tooltip.GetToolTip(this) != tip) tooltip.SetToolTip(this, tip);
    }
    protected override void OnMouseLeave(EventArgs e) { base.OnMouseLeave(e); if (hovered != null) hovered.Background = previousBackground; hovered = null; RefreshFrame(scale); }
    protected override void WndProc(ref Forms.Message message) { if (message.Msg == 0x0010) { close(); return; } base.WndProc(ref message); }
    protected override void Dispose(bool disposing) { if (disposing) { frame?.Dispose(); frame = null; tooltip.Dispose(); } base.Dispose(disposing); }
}
