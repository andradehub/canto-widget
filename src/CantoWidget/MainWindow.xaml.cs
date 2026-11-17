using System;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text.Json;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;
using Microsoft.Win32;
using Forms = System.Windows.Forms;

namespace CantoWidget;

public partial class MainWindow : Window
{
    private readonly MediaService media = new();
    private readonly MouseBattery mouse = new();
    private readonly DispatcherTimer timer = new() { Interval = TimeSpan.FromSeconds(2) };
    private readonly string[] args;
    private readonly Preferences preferences = Preferences.Load();
    private TaskbarHost? taskbarHost;
    private Forms.NotifyIcon? tray;
    private Process? syncProcess;
    private bool connecting;
    private bool refreshing;
    private DateTimeOffset nextBatteryPoll;
    private BatteryState battery = new(null, false, "AJAZZ AJ139 Pro", "Consultando receptor…", null);
    private string? repository;
    private bool closing;
    public MainWindow(string[] arguments)
    {
        InitializeComponent(); args = arguments;
        Loaded += OnLoaded;
        SourceInitialized += (_, _) => { var helper = new System.Windows.Interop.WindowInteropHelper(this); var style = NativeWindow.GetWindowLongPtr(helper.Handle, -20).ToInt64(); NativeWindow.SetWindowLongPtr(helper.Handle, -20, new IntPtr(style | 0x80 | 0x08000000)); var source = System.Windows.Interop.HwndSource.FromHwnd(helper.Handle); if (source?.CompositionTarget != null) source.CompositionTarget.RenderMode = System.Windows.Interop.RenderMode.SoftwareOnly; };
        Microsoft.Win32.SystemEvents.DisplaySettingsChanged += DisplayChanged;
        Microsoft.Win32.SystemEvents.UserPreferenceChanged += PreferencesChanged;
        timer.Tick += async (_, _) => await RefreshAsync();
        Closed += (_, _) => { closing = true; timer.Stop(); taskbarHost?.Dispose(); tray?.Dispose(); StopSync(); Microsoft.Win32.SystemEvents.DisplaySettingsChanged -= DisplayChanged; Microsoft.Win32.SystemEvents.UserPreferenceChanged -= PreferencesChanged; Application.Current.Shutdown(); };
    }
    private async void OnLoaded(object sender, RoutedEventArgs e)
    {
        taskbarHost = new TaskbarHost(this);
        taskbarHost.Start();
        repository = FindRepository();
        RefreshStartupLabel();
        SetupTray();
        if (preferences.AutoSync) StartSync();
        await RefreshAsync();
        timer.Start();
        int captureIndex = Array.IndexOf(args, "--capture");
        if (captureIndex >= 0 && captureIndex + 1 < args.Length)
        {
            await Task.Delay(2500); await RefreshAsync();
            Card.UpdateLayout();
            var bitmap = new RenderTargetBitmap((int)Math.Ceiling(ActualWidth * 2), (int)Math.Ceiling(ActualHeight * 2), 192, 192, PixelFormats.Pbgra32);
            bitmap.Render(this); var png = new PngBitmapEncoder(); png.Frames.Add(BitmapFrame.Create(bitmap));
            using (var stream = File.Create(args[captureIndex + 1])) png.Save(stream);
            Application.Current.Shutdown();
        }
    }
    private async Task RefreshAsync()
    {
        if (refreshing || closing) return;
        refreshing = true;
        try
        {
            Task<BatteryState>? batteryTask = null;
            if (DateTimeOffset.Now >= nextBatteryPoll) { nextBatteryPoll = DateTimeOffset.Now.AddSeconds(30); batteryTask = mouse.ReadAsync(); }
            await media.RefreshAsync();
            if (batteryTask != null) battery = await batteryTask;
            if (closing) return;
            var music = media.State;
            SongTitle.Text = music.Title; Artist.Text = music.Artist; Cover.Source = media.Cover;
            SongTitle.ToolTip = music.Title + "\n" + music.Artist;
            Play.IsEnabled = Previous.IsEnabled = Next.IsEnabled = music.Available;
            PlayIcon.Data = Geometry.Parse(music.Playing ? "M 0,0 L 3,0 L 3,12 L 0,12 Z M 7,0 L 10,0 L 10,12 L 7,12 Z" : "M 0,0 L 0,12 L 9,6 Z");
            ProgressLine.Width = Math.Max(0, (Card.ActualWidth - 28) * music.Progress);
            BatteryText.Text = battery.Percent.HasValue ? $"{battery.Percent}%" + (battery.Charging ? " ⚡" : "") : "—";
            BatteryText.Foreground = new SolidColorBrush(battery.Percent is <= 20 ? Color.FromRgb(255, 182, 123) : Color.FromRgb(225, 241, 248));
            BatteryDot.Fill = new SolidColorBrush(battery.Percent == null ? Color.FromRgb(120, 152, 169) : battery.Message.StartsWith("Última") || battery.Percent <= 20 ? Color.FromRgb(255, 182, 123) : Color.FromRgb(133, 228, 207));
            BatteryArea.ToolTip = battery.Device + "\n" + battery.Message + (battery.UpdatedAt.HasValue ? $"\nLeitura às {battery.UpdatedAt:HH:mm:ss}" : "");
            DetailPercent.Text = BatteryText.Text;
            BatteryDetail.Text = battery.Message + (battery.UpdatedAt.HasValue ? $"\nAtualizado às {battery.UpdatedAt:HH:mm:ss}" : "");
            UpdateSyncStatus();
            TaskbarStatus.Text = taskbarHost?.Status ?? "Conectando à barra de tarefas…";
        }
        catch (Exception ex) { AppLog.Write(ex.ToString()); }
        finally { refreshing = false; }
    }
    private async void PreviousClick(object sender, RoutedEventArgs e) => await Control("previous");
    private async void PlayClick(object sender, RoutedEventArgs e) => await Control("play");
    private async void NextClick(object sender, RoutedEventArgs e) => await Control("next");
    private async Task Control(string command)
    {
        if (!await media.CommandAsync(command)) { Artist.Text = "O player não aceitou o comando"; return; }
        await Task.Delay(300); await RefreshAsync();
    }
    private void OptionsClick(object sender, RoutedEventArgs e) { UpdateSyncStatus(); Options.IsOpen = !Options.IsOpen; }
    private async void RefreshBatteryClick(object sender, RoutedEventArgs e) { nextBatteryPoll = DateTimeOffset.MinValue; await RefreshAsync(); }
    private void Dock()
    {
        taskbarHost?.Layout();
    }
    private void DisplayChanged(object? sender, EventArgs e) => Dispatcher.BeginInvoke(() => { preferences.Left = preferences.Top = null; Dock(); preferences.Save(); });
    private void PreferencesChanged(object sender, UserPreferenceChangedEventArgs e) { if (preferences.Left == null) Dispatcher.BeginInvoke(Dock); }
    private void ResetPositionClick(object sender, RoutedEventArgs e) { preferences.Left = preferences.Top = null; preferences.Save(); Options.IsOpen = false; Dock(); }
    private void SetupTray()
    {
        tray = new Forms.NotifyIcon { Icon = System.Drawing.SystemIcons.Information, Text = "Canto · música e bateria do mouse", Visible = true };
        var menu = new Forms.ContextMenuStrip();
        menu.Items.Add("Mostrar Canto", null, (_, _) => Dispatcher.Invoke(() => { Show(); Dock(); }));
        menu.Items.Add("Opções", null, (_, _) => Dispatcher.Invoke(() => { Show(); Options.IsOpen = true; }));
        menu.Items.Add("Sair", null, (_, _) => Dispatcher.Invoke(() => Application.Current.Shutdown()));
        tray.ContextMenuStrip = menu;
        tray.DoubleClick += (_, _) => Dispatcher.Invoke(() => { Show(); Dock(); });
    }
    private void RefreshStartupLabel()
    {
        using var key = Registry.CurrentUser.OpenSubKey(@"Software\Microsoft\Windows\CurrentVersion\Run");
        StartupButton.Content = key?.GetValue("CantoWidget") != null ? "✓  Iniciar com o Windows" : "Iniciar com o Windows";
    }
    private void StartupClick(object sender, RoutedEventArgs e)
    {
        try
        {
            using var key = Registry.CurrentUser.CreateSubKey(@"Software\Microsoft\Windows\CurrentVersion\Run");
            if (key.GetValue("CantoWidget") != null) key.DeleteValue("CantoWidget"); else key.SetValue("CantoWidget", "\"" + Environment.ProcessPath + "\"");
            RefreshStartupLabel();
        }
        catch (Exception ex) { AppLog.Write(ex.Message); }
    }
    private static string? FindRepository()
    {
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir != null) { if (File.Exists(Path.Combine(dir.FullName, "scripts", "Sync-GitHub.ps1"))) return dir.FullName; dir = dir.Parent; }
        return null;
    }
    private void StartSync()
    {
        if (connecting || repository == null || !Directory.Exists(Path.Combine(repository, ".git"))) return;
        string stopFile = Path.Combine(AppLog.DirectoryPath, "sync-" + RepoKey(repository) + ".stop");
        try { File.Delete(stopFile); } catch (Exception ex) { AppLog.Write(ex.Message); }
        if (syncProcess is { HasExited: false }) return;
        try
        {
            var start = new ProcessStartInfo("powershell.exe") { UseShellExecute = false, CreateNoWindow = true, WindowStyle = ProcessWindowStyle.Hidden };
            foreach (var item in new[] { "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", Path.Combine(repository, "scripts", "Sync-GitHub.ps1"), "-Repository", repository }) start.ArgumentList.Add(item);
            syncProcess = Process.Start(start);
        }
        catch (Exception ex) { AppLog.Write("Sincronização: " + ex.Message); }
    }
    private void StopSync()
    {
        try
        {
            if (repository != null) { Directory.CreateDirectory(AppLog.DirectoryPath); File.WriteAllText(Path.Combine(AppLog.DirectoryPath, "sync-" + RepoKey(repository) + ".stop"), "stop"); }
            // Let an in-progress commit/push finish; the watcher exits at its next poll.
        }
        catch (Exception ex) { AppLog.Write(ex.Message); }
    }
    private void SyncClick(object sender, RoutedEventArgs e) { preferences.AutoSync = !preferences.AutoSync; preferences.Save(); if (preferences.AutoSync) StartSync(); else StopSync(); UpdateSyncStatus(); }
    private void ConnectGithubClick(object sender, RoutedEventArgs e)
    {
        if (repository == null) { GithubStatus.Text = "Mantenha o aplicativo dentro da pasta do projeto."; return; }
        if (connecting) return;
        connecting = true;
        StopSync();
        try
        {
            var start = new ProcessStartInfo("powershell.exe") { UseShellExecute = true, WorkingDirectory = repository };
            // This window is interactive: the user needs the login code and the browser.
            start.Arguments = "-NoProfile -ExecutionPolicy Bypass -File \"" + Path.Combine(repository, "scripts", "Connect-GitHub.ps1") + "\"";
            var connection = Process.Start(start);
            if (connection != null) { connection.EnableRaisingEvents = true; connection.Exited += (_, _) => Dispatcher.BeginInvoke(() => { connecting = false; connection.Dispose(); if (!closing && preferences.AutoSync) StartSync(); }); }
            else connecting = false;
            GithubStatus.Text = "conclua o login na janela de conexão";
        }
        catch (Exception ex) { connecting = false; AppLog.Write(ex.Message); }
    }
    private void UpdateSyncStatus()
    {
        SyncButton.Content = "Sincronização automática: " + (preferences.AutoSync ? "ligada" : "pausada");
        GithubStatus.Text = "GitHub · conecte sua conta para publicar";
        if (connecting) { GithubStatus.Text = "GitHub · conclua o login na janela de conexão"; return; }
        if (!preferences.AutoSync) { GithubStatus.Text = "GitHub · sincronização pausada"; return; }
        if (repository == null) { GithubStatus.Text = "GitHub · abra o aplicativo dentro da pasta do projeto"; return; }
        StartSync();
        try
        {
            string statusPath = Path.Combine(AppLog.DirectoryPath, "sync-" + RepoKey(repository) + ".json");
            if (File.Exists(statusPath)) { using var doc = JsonDocument.Parse(File.ReadAllText(statusPath)); GithubStatus.Text = "GitHub · " + doc.RootElement.GetProperty("message").GetString(); }
        }
        catch (Exception ex) { AppLog.Write(ex.Message); }
    }
    internal static string RepoKey(string path) => Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(System.Text.Encoding.UTF8.GetBytes(Path.GetFullPath(path).TrimEnd(Path.DirectorySeparatorChar).ToLowerInvariant())))[..12].ToLowerInvariant();
    private void RepositoryClick(object sender, RoutedEventArgs e) => Process.Start(new ProcessStartInfo("https://github.com/andradehub/canto-widget") { UseShellExecute = true });
    private void ExitClick(object sender, RoutedEventArgs e) => Application.Current.Shutdown();
}

internal sealed class Preferences
{
    public double? Left { get; set; }
    public double? Top { get; set; }
    public bool AutoSync { get; set; } = true;
    private static string PathName => Path.Combine(AppLog.DirectoryPath, "settings.json");
    public static Preferences Load() { try { return JsonSerializer.Deserialize<Preferences>(File.ReadAllText(PathName)) ?? new(); } catch { return new(); } }
    public void Save() { try { Directory.CreateDirectory(AppLog.DirectoryPath); File.WriteAllText(PathName, JsonSerializer.Serialize(this)); } catch (Exception ex) { AppLog.Write(ex.Message); } }
}

internal static class NativeWindow
{
    [System.Runtime.InteropServices.DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")] internal static extern IntPtr GetWindowLongPtr(IntPtr window, int index);
    [System.Runtime.InteropServices.DllImport("user32.dll", EntryPoint = "SetWindowLongPtrW")] internal static extern IntPtr SetWindowLongPtr(IntPtr window, int index, IntPtr value);
}
