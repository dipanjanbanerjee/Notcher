; Ask Explorer to reload icons after installing, so desktop and Start menu
; shortcuts show the current logo instead of a cached old one.
!macro NSIS_HOOK_POSTINSTALL
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0x1000, p 0, p 0)'
!macroend
