// Compile as Weixin.exe for the synthetic taskbar signal integration test.
using System;
using System.Runtime.InteropServices;
using System.Windows.Forms;

sealed class TaskbarFlash : Form
{
    [StructLayout(LayoutKind.Sequential)]
    struct FlashInfo
    {
        public uint Size;
        public IntPtr Window;
        public uint Flags, Count, Timeout;
    }

    [DllImport("user32.dll")]
    static extern bool FlashWindowEx(ref FlashInfo info);
    protected override bool ShowWithoutActivation { get { return true; } }

    [STAThread]
    static void Main()
    {
        using (var window = new TaskbarFlash { Text = "DotIsle shell flash fixture", Width = 300, Height = 100 })
        using (var timer = new Timer { Interval = 900 })
        {
            timer.Tick += delegate {
                timer.Stop();
                var info = new FlashInfo { Size = (uint)Marshal.SizeOf(typeof(FlashInfo)),
                    Window = window.Handle, Flags = 2, Count = 3 };
                FlashWindowEx(ref info);
            };
            timer.Start();
            Application.Run(window);
        }
    }
}
