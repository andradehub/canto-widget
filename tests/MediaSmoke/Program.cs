using System;
using System.IO;
using System.Threading.Tasks;
using CantoWidget;
using Windows.Media;
using Windows.Media.Core;
using Windows.Media.Playback;

string wav = Path.Combine(AppContext.BaseDirectory, "silence.wav");
using (var writer = new BinaryWriter(File.Create(wav)))
{
    const int samples = 8000 * 30;
    writer.Write(System.Text.Encoding.ASCII.GetBytes("RIFF")); writer.Write(36 + samples * 2);
    writer.Write(System.Text.Encoding.ASCII.GetBytes("WAVEfmt ")); writer.Write(16); writer.Write((short)1); writer.Write((short)1);
    writer.Write(8000); writer.Write(16000); writer.Write((short)2); writer.Write((short)16);
    writer.Write(System.Text.Encoding.ASCII.GetBytes("data")); writer.Write(samples * 2); writer.Write(new byte[samples * 2]);
}
using var player = new MediaPlayer();
player.Volume = 0;
player.CommandManager.IsEnabled = false;
player.Source = MediaSource.CreateFromUri(new Uri(wav));
var controls = player.SystemMediaTransportControls;
controls.IsEnabled = true; controls.IsPlayEnabled = true; controls.IsPauseEnabled = true;
controls.DisplayUpdater.Type = MediaPlaybackType.Music;
controls.DisplayUpdater.MusicProperties.Title = "Canto — teste de integração";
controls.DisplayUpdater.MusicProperties.Artist = "Sessão de teste silenciosa";
controls.DisplayUpdater.Update();
controls.ButtonPressed += (_, e) => { if (e.Button == SystemMediaTransportControlsButton.Pause) { player.Pause(); controls.PlaybackStatus = MediaPlaybackStatus.Paused; } else if (e.Button == SystemMediaTransportControlsButton.Play) { player.Play(); controls.PlaybackStatus = MediaPlaybackStatus.Playing; } };
player.Play(); controls.PlaybackStatus = MediaPlaybackStatus.Playing;
var service = new MediaService();
for (int i = 0; i < 15; i++) { await Task.Delay(500); await service.RefreshAsync(); if (service.State.Title.Contains("teste de integração")) break; }
if (!service.State.Title.Contains("teste de integração") || !service.State.Playing) throw new Exception("A sessão real de mídia não foi lida: " + service.State);
Console.WriteLine("PASS título, artista e estado lidos da sessão real do Windows");
if (!await service.CommandAsync("play")) throw new Exception("Comando de pausa não aceito");
await Task.Delay(1000); await service.RefreshAsync();
if (service.State.Playing) throw new Exception("O comando não pausou o player");
Console.WriteLine("PASS controle do widget pausou o player pela API do Windows");
controls.IsEnabled = false; player.Source = null;

namespace CantoWidget { internal static class AppLog { public static void Write(string message) => Console.WriteLine(message); } }
