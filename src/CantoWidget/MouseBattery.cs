using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.Win32.SafeHandles;

namespace CantoWidget;

public record BatteryState(int? Percent, bool Charging, string Device, string Message, DateTimeOffset? UpdatedAt);

// Only status queries. The protocol never changes DPI, buttons, lighting or firmware.
public sealed class MouseBattery
{
    private BatteryState? last;
    public async Task<BatteryState> ReadAsync()
    {
        bool found = false;
        foreach (string path in HidNative.Paths().OrderBy(path => path.Contains("vid_248a&pid_5", StringComparison.OrdinalIgnoreCase) && !path.Contains("pid_5c2f", StringComparison.OrdinalIgnoreCase) ? 0 : 1))
        {
            bool wireless = path.Contains("vid_249a&pid_5c2f", StringComparison.OrdinalIgnoreCase) || path.Contains("vid_248a&pid_5c2f", StringComparison.OrdinalIgnoreCase);
            bool wired = path.Contains("vid_248a&pid_5c2e", StringComparison.OrdinalIgnoreCase) || path.Contains("vid_248a&pid_5d2e", StringComparison.OrdinalIgnoreCase) || path.Contains("vid_248a&pid_5e2e", StringComparison.OrdinalIgnoreCase);
            if ((!wireless && !wired) || !path.Contains("mi_02", StringComparison.OrdinalIgnoreCase)) continue;
            found = true;
            try
            {
                using var handle = HidNative.CreateFile(path, 0xC0000000, 3, IntPtr.Zero, 3, 0x40000000, IntPtr.Zero);
                if (handle.IsInvalid) { AppLog.Write("Mouse: canal HID ocupado ou sem acesso."); continue; }
                if (!HidNative.TryGetCaps(handle, out var caps)) continue;
                AppLog.Write($"Mouse HID: entrada={caps.InputReportByteLength}, saída={caps.OutputReportByteLength}, uso={caps.UsagePage:X4}:{caps.Usage:X4}");
                if (caps.OutputReportByteLength != 33 || caps.InputReportByteLength != 33) continue;
                using var stream = new FileStream(handle, FileAccess.ReadWrite, 1, true);
                using var budget = new CancellationTokenSource(TimeSpan.FromSeconds(2));
                byte[] request = new byte[33]; request[1] = 0x10;
                await stream.WriteAsync(request, budget.Token);
                await stream.FlushAsync(budget.Token);
                var until = Stopwatch.StartNew();
                BatteryState? candidate = null;
                while (until.ElapsedMilliseconds < 1300)
                {
                    byte[] reply = new byte[33];
                    using var readBudget = CancellationTokenSource.CreateLinkedTokenSource(budget.Token);
                    readBudget.CancelAfter(350);
                    int length;
                    try { length = await stream.ReadAsync(reply, readBudget.Token); }
                    catch (OperationCanceledException) { break; }
                    AppLog.Write("Mouse status: " + Convert.ToHexString(reply.AsSpan(0, length)));
                    var parsed = AjazzProtocol.Parse(reply.AsSpan(0, length));
                    if (parsed == null) continue;
                    candidate = new(parsed.Value.Percent, wired && parsed.Value.Charging == true, "AJAZZ AJ139 Pro", "Conectado · " + (wireless ? "receptor USB" : "cabo USB"), DateTimeOffset.Now);
                    if (parsed.Value.IsInfo) break;
                }
                if (candidate != null) { last = candidate; return candidate; }
            }
            catch (Exception ex) { AppLog.Write("Mouse: " + ex.Message); }
        }
        if (found && last?.UpdatedAt != null && DateTimeOffset.Now - last.UpdatedAt < TimeSpan.FromMinutes(3))
            return last with { Message = "Última leitura · mouse sem resposta" };
        last = null;
        return new(null, false, "AJAZZ AJ139 Pro", found ? "Sem leitura · mova o mouse para acordá-lo" : "Receptor não encontrado", null);
    }
}

public static class AjazzProtocol
{
    public readonly record struct Reading(int Percent, bool? Charging, bool IsInfo);
    public static Reading? Parse(ReadOnlySpan<byte> data)
    {
        if (data.Length == 33 && data[0] == 0) data = data[1..];
        if (data.Length != 32) return null;
        if (data[0] == 0xC0 && data[1] == 1 && data[2] is > 0 and <= 100) return new(data[2], null, false);
        if (data[0] != 0x10 || data[1] != 0 || data[2] != 1 || data[3] != 11 || data[13] is 0 or > 100 || data[14] > 1) return null;
        int checksum = 0; for (int i = 4; i < 31; i++) checksum += data[i];
        return (byte)checksum == data[31] ? new Reading(data[13], data[14] == 0, true) : null;
    }
}

internal static class HidNative
{
    [StructLayout(LayoutKind.Sequential)] internal struct InterfaceData { public int Size; public Guid Guid; public int Flags; public IntPtr Reserved; }
    [StructLayout(LayoutKind.Sequential)] internal struct Caps
    {
        public ushort Usage, UsagePage, InputReportByteLength, OutputReportByteLength, FeatureReportByteLength;
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 17)] public ushort[] Reserved;
        public ushort NumberLinkCollectionNodes, NumberInputButtonCaps, NumberInputValueCaps, NumberInputDataIndices, NumberOutputButtonCaps, NumberOutputValueCaps, NumberOutputDataIndices, NumberFeatureButtonCaps, NumberFeatureValueCaps, NumberFeatureDataIndices;
    }
    [DllImport("hid.dll")] static extern void HidD_GetHidGuid(out Guid guid);
    [DllImport("hid.dll")] static extern bool HidD_GetPreparsedData(SafeFileHandle handle, out IntPtr data);
    [DllImport("hid.dll")] static extern bool HidD_FreePreparsedData(IntPtr data);
    [DllImport("hid.dll")] static extern int HidP_GetCaps(IntPtr data, out Caps caps);
    [DllImport("setupapi.dll", CharSet = CharSet.Unicode)] static extern IntPtr SetupDiGetClassDevsW(ref Guid guid, string? enumerator, IntPtr parent, int flags);
    [DllImport("setupapi.dll", SetLastError = true)] static extern bool SetupDiEnumDeviceInterfaces(IntPtr set, IntPtr device, ref Guid guid, int index, ref InterfaceData data);
    [DllImport("setupapi.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern bool SetupDiGetDeviceInterfaceDetailW(IntPtr set, ref InterfaceData data, IntPtr detail, int length, out int needed, IntPtr device);
    [DllImport("setupapi.dll")] static extern bool SetupDiDestroyDeviceInfoList(IntPtr set);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)] internal static extern SafeFileHandle CreateFile(string path, uint access, uint share, IntPtr security, uint creation, uint flags, IntPtr template);
    internal static bool TryGetCaps(SafeFileHandle handle, out Caps caps)
    {
        caps = default;
        if (!HidD_GetPreparsedData(handle, out var data)) return false;
        try { return HidP_GetCaps(data, out caps) >= 0; } finally { HidD_FreePreparsedData(data); }
    }
    internal static IEnumerable<string> Paths()
    {
        HidD_GetHidGuid(out var guid);
        var set = SetupDiGetClassDevsW(ref guid, null, IntPtr.Zero, 0x12);
        if (set == new IntPtr(-1)) yield break;
        try
        {
            for (int i = 0; ; i++)
            {
                var data = new InterfaceData { Size = Marshal.SizeOf<InterfaceData>() };
                if (!SetupDiEnumDeviceInterfaces(set, IntPtr.Zero, ref guid, i, ref data)) yield break;
                SetupDiGetDeviceInterfaceDetailW(set, ref data, IntPtr.Zero, 0, out int size, IntPtr.Zero);
                if (size < 8) continue;
                var buffer = Marshal.AllocHGlobal(size);
                string? path = null;
                try { Marshal.WriteInt32(buffer, IntPtr.Size == 8 ? 8 : 6); if (SetupDiGetDeviceInterfaceDetailW(set, ref data, buffer, size, out _, IntPtr.Zero)) path = Marshal.PtrToStringUni(buffer + 4); }
                finally { Marshal.FreeHGlobal(buffer); }
                if (path != null) yield return path;
            }
        }
        finally { SetupDiDestroyDeviceInfoList(set); }
    }
}
