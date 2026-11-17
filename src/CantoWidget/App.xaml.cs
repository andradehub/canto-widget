using System;
using System.IO;
using System.Linq;
using System.Threading;
using System.Windows;

namespace CantoWidget;

public partial class App : Application
{
    private Mutex? instance;
    protected override async void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);
        DispatcherUnhandledException += (_, args) => { AppLog.Write(args.Exception.ToString()); args.Handled = true; };
        if (e.Args.Contains("--diagnostics"))
        {
            try
            {
                var battery = await new MouseBattery().ReadAsync();
                var media = new MediaService();
                await media.RefreshAsync();
                var result = System.Text.Json.JsonSerializer.Serialize(new { battery, music = media.State }, new System.Text.Json.JsonSerializerOptions { WriteIndented = true });
                Directory.CreateDirectory(AppLog.DirectoryPath);
                File.WriteAllText(e.Args.SkipWhile(x => x != "--diagnostics").Skip(1).FirstOrDefault() ?? Path.Combine(AppLog.DirectoryPath, "diagnostics.json"), result);
            }
            catch (Exception ex) { AppLog.Write(ex.ToString()); Environment.ExitCode = 1; }
            Shutdown(); return;
        }
        instance = new Mutex(true, "Local\\CantoWidget.Desktop", out bool isNew);
        if (!isNew && !e.Args.Contains("--preview")) { Shutdown(); return; }
        var window = new MainWindow(e.Args);
        MainWindow = window;
        window.Show();
    }
    protected override void OnExit(ExitEventArgs e) { instance?.Dispose(); base.OnExit(e); }
}

internal static class AppLog
{
    public static string DirectoryPath { get; } = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "CantoWidget");
    public static void Write(string message)
    {
        try { Directory.CreateDirectory(DirectoryPath); var path = Path.Combine(DirectoryPath, "canto.log"); if (File.Exists(path) && new FileInfo(path).Length > 1_000_000) File.WriteAllText(path, ""); File.AppendAllText(path, $"{DateTimeOffset.Now:O} {message}{Environment.NewLine}"); } catch { }
    }
}
