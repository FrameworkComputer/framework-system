;; This is an AutoHotKey -*- ahk -*- script 
;;
;; ABOUT
;;  Respond to WM_SETTINGCHANGE messages and 
;;
;; USAGE
;;  Run the script directly (e.g. double-click) or drag and drop onto
;;  the AutoHotKey application.

;; Keep it running persistently to wait for WM_SETTINGCHANGE events
;; Stays in the taskbar tray and can be stopped from there
Persistent

;;
;; Register an AHK function as a callback.
;;
OnMessage((WM_SETTINGCHANGE:=0x1A), recv_WM_SETTINGCHANGE)

;;
;; The WM_SETTINGCHANGE callback
;;
recv_WM_SETTINGCHANGE(wParam, lParam, msg, hwnd)
{
  ;; MsgBox "Settings changed. lparam: " lparam
  If lparam != 0 {
    lparam_str := StrGet(lparam, "UTF-16")
    ;; MsgBox Format("LPARAM: {1}", lparam_str), "WM_SETTINGCHANGE"
    ;; System switched between tablet and laptop mode
    If lparam_str == "ConvertibleSlateMode" {
      ;; MsgBox "Toggle touchpad"
      ;; CTRL+WIN+F24 to toggle touchpad
      Send "^#{F24}"
    }
  }
}
