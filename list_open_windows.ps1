$ErrorActionPreference = "STOP"
$env:SHELL = "powershell"

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Collections.Generic;
using System.Text;
using System.IO;

public static class WindowFinder
{
	private const uint GA_ROOTOWNER = 3;
	static readonly int GWL_STYLE = -16;
	static readonly int GWL_EXSTYLE = -20;

	private const uint WS_CHILD = 0x40000000;
	private const uint WS_EX_NOACTIVATE = 0x08000000;

	public enum GetAncestorFlags
	{
			GetRootOwner = 3
	}

	[DllImport("user32.dll", SetLastError = false)]
	static extern IntPtr GetDesktopWindow();

	[DllImport("user32.dll")]
	static extern IntPtr GetLastActivePopup(IntPtr hWnd);

	[DllImport("user32.dll")]
	[return: MarshalAs(UnmanagedType.Bool)]
	static extern bool IsWindowVisible(IntPtr hWnd);

	[DllImport("user32.dll", SetLastError=true)]
	private static extern int GetWindowThreadProcessId(IntPtr hWnd, out int lpdwProcessId);

	[DllImport("user32.dll", ExactSpelling = true)]
	static extern IntPtr GetAncestor(IntPtr hwnd, GetAncestorFlags flags);

	[DllImport("user32.dll", ExactSpelling=true, CharSet=CharSet.Auto)]
	private static extern IntPtr GetParent(IntPtr hWnd);

	[DllImport("kernel32.dll", SetLastError=true)]
	private static extern bool CloseHandle(
			IntPtr hObject);

	[DllImport("user32.dll")]
	private static extern int GetWindowLong(IntPtr hWnd, int nIndex);

	[DllImport("kernel32.dll", SetLastError = true)]
	private static extern IntPtr OpenProcess(
			int processAccess,
			bool bInheritHandle,
			int processId);

	[DllImport("kernel32.dll", SetLastError=true)]
	private static extern bool QueryFullProcessImageName(
			IntPtr hProcess,
			int dwFlags,
			System.Text.StringBuilder lpExeName,
			ref int lpdwSize);

	[DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Auto)]
	static extern int GetClassName(IntPtr hWnd, StringBuilder lpClassName,int nMaxCount);

	[DllImport("user32.dll")]
	private static extern IntPtr GetWindowText(IntPtr hWnd, System.Text.StringBuilder text, int count);

	[DllImport("user32.dll")]
	private static extern bool EnumWindows(EnumWindowsProc enumProc, System.IntPtr lParam);

	public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

	private const int QueryLimitedInformation = 0x00001000;

	private static bool IsAlTabWindow(IntPtr hWnd)
	{
		var desktopWindow = GetDesktopWindow();
		if(hWnd == desktopWindow)
		{
			return false;
		}

		var isWindowVisible = IsWindowVisible(hWnd);

		if(!isWindowVisible)
		{
			return false;
		}

		int styles = GetWindowLong(hWnd, GWL_STYLE);
		if((styles & WS_CHILD) != 0)
		{
			return false;
		}

		int exStyles = GetWindowLong(hWnd, GWL_EXSTYLE);
		if((exStyles & WS_EX_NOACTIVATE) != 0)
		{
			return false;
		}

		var parent = GetParent(hWnd);
		if(parent != IntPtr.Zero)
		{
			return false;
		}
		
		return true;
	}

	private static int GetProcessId(IntPtr hWnd)
	{
		int processID = 0;
		int threadID = GetWindowThreadProcessId(hWnd, out processID);
		return processID;
	}

	private static string GetProcessPath(int processID)
	{
		var size = 1024;
		var sb = new StringBuilder(size);
		var handle = OpenProcess(QueryLimitedInformation, false, (int)processID);
		if (handle == IntPtr.Zero)
		{
			return null;
		}
		var success = QueryFullProcessImageName(handle, 0, sb, ref size);
		CloseHandle(handle);
		if (!success)
		{
			return null;
		}
		return sb.ToString();
	}

	private static bool is_alt_tab_window(IntPtr hwnd)
	{
			// Start at the root owner
			IntPtr hwndWalk = GetAncestor(hwnd, GetAncestorFlags.GetRootOwner);
			// See if we are the last active visible popup
			IntPtr hwndTry;
			while ((hwndTry = GetLastActivePopup(hwndWalk)) != hwndTry)
			{
					if (IsWindowVisible(hwndTry)) break;
					hwndWalk = hwndTry;
			}
			return hwndWalk == hwnd;
	}

	public static IEnumerable<object> FindWindows()
	{
		IntPtr found = IntPtr.Zero;
		var windows = new List<object>();

		EnumWindows(delegate(IntPtr hwnd, IntPtr param)
				{
					if(IsAlTabWindow(hwnd))
					{
						if(is_alt_tab_window(hwnd))
						{
							var classNameSb = new System.Text.StringBuilder(256);
							var classNameResult = GetClassName(hwnd, classNameSb, 256);

							var titleStringBuilder = new StringBuilder(256);
							var count = GetWindowText(hwnd, titleStringBuilder, 256);
							if(count != IntPtr.Zero && classNameResult != 0)
							{
								var className = classNameSb.ToString();
								var title = titleStringBuilder.ToString();
								if(className.Contains("Progman"))
								{
										return true;
								}

								if(title.Contains("ApplicationFrameWindow"))
								{
										return true;
								}

								var processId = GetProcessId(hwnd);
								var processPath = GetProcessPath(processId);
								var processName = Path.GetFileName(processPath);
								var result = new { hwnd = hwnd.ToString("X16"), PID = processId.ToString("D8"), Title = title, ProcessName = processName };
								windows.Add(result);
							}
						}
					}

					return true;
				},
				IntPtr.Zero);

		return windows;
	}
}
'@

function GetWindows {
	$windows = [WindowFinder]::FindWindows()
	$header1 = "1 Kill Process:  ctrl-k"
	$header2 = "1 Close Process: ctrl-c"
	$header3 = "1 Reload:        ctrl-r"
	$header1,$header2,$header3,$windows
}

function Run {
	$result = GetWindows | Fzf `
		--with-nth 2.. `
		--header-lines=6 `
		--bind="ctrl-r:reload(. .\list_open_windows.ps1;GetWindows)" `
		--bind="ctrl-c:execute(taskkill /pid {2})+reload(. .\list_open_windows.ps1;GetWindows)" `
		--bind="ctrl-k:execute(taskkill /f /pid {2})+reload(. .\list_open_windows.ps1;GetWindows)" `
		--reverse
	$result
}
