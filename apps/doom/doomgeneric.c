#include "m_argv.h"
#include "doomgeneric.h"

pixel_t* DG_ScreenBuffer = NULL;

void M_FindResponseFile(void);
void D_DoomMain (void);
extern void DG_Init(void);

void doomgeneric_Create(int argc, char **argv)
{
	// save arguments
    myargc = argc;
    myargv = argv;

	M_FindResponseFile();

	// DG_ScreenBuffer is set by the Rust wrapper before calling doomgeneric_Create
	// or we can set it here if we know the address. 
	// But the Rust wrapper already sets it.

	DG_Init();

	D_DoomMain ();
}

