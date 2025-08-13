configFile = SampleConfig.c
requireAdmin = FALSE
nfmSourceDir = $(USERPROFILE)\src\nfzf
!if exists(props.mk.local)
include props.mk.local
!endif
