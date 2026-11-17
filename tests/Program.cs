using System;
using CantoWidget;

static void Check(bool test, string name) { if (!test) throw new Exception(name); Console.WriteLine("PASS " + name); }
byte[] status = new byte[32];
status[0] = 0x10; status[2] = 1; status[3] = 11;
status[4] = (byte)'N'; status[5] = (byte)'6'; status[6] = (byte)'2'; status[7] = (byte)'5';
status[13] = 75; status[14] = 1;
void Checksum() { int sum = 0; for (int i = 4; i < 31; i++) sum += status[i]; status[31] = (byte)sum; }
Checksum();
Check(AjazzProtocol.Parse(status)?.Percent == 75, "status: 75%");
Check(AjazzProtocol.Parse(status)?.Charging == false, "wireless: sem carga");
status[14] = 0; Checksum(); Check(AjazzProtocol.Parse(status)?.Charging == true, "cabo: carregando");
byte[] withId = new byte[33]; status.CopyTo(withId, 1); Check(AjazzProtocol.Parse(withId)?.Percent == 75, "report ID do Windows");
status[31] ^= 1; Check(AjazzProtocol.Parse(status) == null, "checksum incorreto rejeitado");
status[13] = 101; Checksum(); Check(AjazzProtocol.Parse(status) == null, "percentual acima de 100 rejeitado");
status[13] = 0; Checksum(); Check(AjazzProtocol.Parse(status) == null, "resposta vazia não vira bateria zero");
Check(AjazzProtocol.Parse(new byte[4]) == null, "frame truncado rejeitado");
byte[] announcement = new byte[32]; announcement[0] = 0xC0; announcement[1] = 1; announcement[2] = 42;
Check(AjazzProtocol.Parse(announcement)?.Percent == 42, "anúncio espontâneo válido");
announcement[1] = 0; Check(AjazzProtocol.Parse(announcement) == null, "anúncio sem link rejeitado");

namespace CantoWidget { internal static class AppLog { public static void Write(string message) { } } }
