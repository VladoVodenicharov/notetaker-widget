; Always put a Notetaker shortcut on the desktop (the Start menu one is created by default).
!macro NSIS_HOOK_POSTINSTALL
  Call CreateOrUpdateDesktopShortcut
!macroend
