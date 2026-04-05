/*
Copyright (C) 1996-1997 Id Software, Inc.

This program is free software; you can redistribute it and/or
modify it under the terms of the GNU General Public License
as published by the Free Software Foundation; either version 2
of the License, or (at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  

See the GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program; if not, write to the Free Software
Foundation, Inc., 59 Temple Place - Suite 330, Boston, MA  02111-1307, USA.

*/
// sys_null.h -- null system driver to aid porting efforts

#include "quakedef.h"
#include "errno.h"

extern int quake_poll_key(void);
extern int quake_poll_scancode(void);
extern void quake_yield(void);
extern unsigned long long quake_uptime_ms(void);

static int QuakeKeyFromScancode(int sc, qboolean extended)
{
	(void)extended;
	switch (sc)
	{
		case 0x01: return K_ESCAPE;
		case 0x0E: return K_BACKSPACE;
		case 0x0F: return K_TAB;
		case 0x1C: return K_ENTER;
		case 0x1D: return K_CTRL;
		case 0x2A:
		case 0x36: return K_SHIFT;
		case 0x38: return K_ALT;
		case 0x39: return K_SPACE;

		case 0x10: return 'w';
		case 0x11: return 'w';
		case 0x12: return 'e';
		case 0x13: return 'r';
		case 0x14: return 't';
		case 0x15: return 'y';
		case 0x16: return 'u';
		case 0x17: return 'i';
		case 0x18: return 'o';
		case 0x19: return 'p';

		case 0x1E: return 'a';
		case 0x1F: return 's';
		case 0x20: return 'd';
		case 0x21: return 'f';
		case 0x22: return 'g';
		case 0x23: return 'h';
		case 0x24: return 'j';
		case 0x25: return 'k';
		case 0x26: return 'l';

		case 0x2C: return 'z';
		case 0x2D: return 'x';
		case 0x2E: return 'c';
		case 0x2F: return 'v';
		case 0x30: return 'b';
		case 0x31: return 'n';
		case 0x32: return 'm';

		case 0x02: return '1';
		case 0x03: return '2';
		case 0x04: return '3';
		case 0x05: return '4';
		case 0x06: return '5';
		case 0x07: return '6';
		case 0x08: return '7';
		case 0x09: return '8';
		case 0x0A: return '9';
		case 0x0B: return '0';

		case 0x48: return K_UPARROW;
		case 0x50: return K_DOWNARROW;
		case 0x4B: return K_LEFTARROW;
		case 0x4D: return K_RIGHTARROW;
		default:   return 0;
	}
}

/*
===============================================================================

FILE IO

===============================================================================
*/

#define MAX_HANDLES             10
FILE    *sys_handles[MAX_HANDLES];

int             findhandle (void)
{
	int             i;
	
	for (i=1 ; i<MAX_HANDLES ; i++)
		if (!sys_handles[i])
			return i;
	Sys_Error ("out of handles");
	return -1;
}

/*
================
filelength
================
*/
int filelength (FILE *f)
{
	int             pos;
	int             end;

	pos = ftell (f);
	fseek (f, 0, SEEK_END);
	end = ftell (f);
	fseek (f, pos, SEEK_SET);

	return end;
}

int Sys_FileOpenRead (char *path, int *hndl)
{
	FILE    *f;
	int             i;
	
	i = findhandle ();

	f = fopen(path, "rb");
	if (!f)
	{
		*hndl = -1;
		return -1;
	}
	sys_handles[i] = f;
	*hndl = i;
	
	return filelength(f);
}

int Sys_FileOpenWrite (char *path)
{
	FILE    *f;
	int             i;
	
	i = findhandle ();

	f = fopen(path, "wb");
	if (!f)
		Sys_Error ("Error opening %s: %s", path,strerror(errno));
	sys_handles[i] = f;
	
	return i;
}

void Sys_FileClose (int handle)
{
	fclose (sys_handles[handle]);
	sys_handles[handle] = NULL;
}

void Sys_FileSeek (int handle, int position)
{
	fseek (sys_handles[handle], position, SEEK_SET);
}

int Sys_FileRead (int handle, void *dest, int count)
{
	return fread (dest, 1, count, sys_handles[handle]);
}

int Sys_FileWrite (int handle, void *data, int count)
{
	return fwrite (data, 1, count, sys_handles[handle]);
}

int     Sys_FileTime (char *path)
{
	FILE    *f;
	
	f = fopen(path, "rb");
	if (f)
	{
		fclose(f);
		return 1;
	}
	
	return -1;
}

void Sys_mkdir (char *path)
{
}


/*
===============================================================================

SYSTEM IO

===============================================================================
*/

void Sys_MakeCodeWriteable (unsigned long startaddr, unsigned long length)
{
}


void Sys_Error (char *error, ...)
{
	va_list         argptr;

	printf ("Sys_Error: ");   
	va_start (argptr,error);
	vprintf (error,argptr);
	va_end (argptr);
	printf ("\n");

	exit (1);
}

void Sys_Printf (char *fmt, ...)
{
	va_list         argptr;
	
	va_start (argptr,fmt);
	vprintf (fmt,argptr);
	va_end (argptr);
}

void Sys_Quit (void)
{
	exit (0);
}

double Sys_FloatTime (void)
{
	return (double)quake_uptime_ms() * 0.001;
}

char *Sys_ConsoleInput (void)
{
	return NULL;
}

void Sys_Sleep (void)
{
}

void Sys_SendKeyEvents (void)
{
	int i;
	qboolean extended = false;

	for (i = 0; i < 128; ++i)
	{
		int sc = quake_poll_scancode();
		int key;
		qboolean down;

		if (sc <= 0)
			break;

		if (sc == 0xE0)
		{
			extended = true;
			continue;
		}

 		down = ((sc & 0x80) == 0);
		key = QuakeKeyFromScancode(sc & 0x7F, extended);
		extended = false;

		if (key)
			Key_Event(key, down);
	}

	/* Fallback for simple cooked ASCII streams when raw scancodes are not delivered. */
	for (i = 0; i < 16; ++i)
	{
		int c = quake_poll_key();
		if (c <= 0)
			break;
		if (c >= 'A' && c <= 'Z')
			c = c - 'A' + 'a';
		Key_Event(c & 0xFF, true);
		Key_Event(c & 0xFF, false);
	}
}

void Sys_HighFPPrecision (void)
{
}

void Sys_LowFPPrecision (void)
{
}

//=============================================================================

void main (int argc, char **argv)
{
	static quakeparms_t    parms;
	double oldtime;

	parms.memsize = 8*1024*1024;
	parms.membase = malloc (parms.memsize);
	parms.basedir = ".";

	COM_InitArgv (argc, argv);

	parms.argc = com_argc;
	parms.argv = com_argv;

	printf ("Host_Init\n");
	Host_Init (&parms);
	oldtime = Sys_FloatTime() - 0.1;
	while (1)
	{
		double newtime = Sys_FloatTime();
		double time = newtime - oldtime;

		if (time < 0)
			time = 0;
		if (time > 0.1)
			time = 0.1;

		Host_Frame (time);
		oldtime = newtime;
		quake_yield();
	}
}


