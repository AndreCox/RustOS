/*
Copyright (C) 1996-1997 Id Software, Inc.

This program is free software; you can redistribute it and/or
modify it under the terms of the GNU General Public License
as published by the Free Software Foundation; either version 2
of the License, or (at your option) any later version.
*/

#include "quakedef.h"
#include "d_local.h"

viddef_t vid;

#define BASEWIDTH  320
#define BASEHEIGHT 200

extern void quake_draw_buffer(const unsigned int *pixels, unsigned int width, unsigned int height);
extern void quake_enter_graphics(void);
extern void quake_exit_graphics(void);

byte vid_buffer[BASEWIDTH * BASEHEIGHT];
short zbuffer[BASEWIDTH * BASEHEIGHT];
byte surfcache[256 * 1024];

unsigned short d_8to16table[256];
unsigned d_8to24table[256];

static unsigned int rgba_buffer[BASEWIDTH * BASEHEIGHT];

void VID_SetPalette(unsigned char *palette)
{
    int i;

    for (i = 0; i < 256; ++i)
    {
        unsigned int r = palette[i * 3 + 0];
        unsigned int g = palette[i * 3 + 1];
        unsigned int b = palette[i * 3 + 2];

        d_8to24table[i] = (r << 16) | (g << 8) | b;
    }
}

void VID_ShiftPalette(unsigned char *palette)
{
    VID_SetPalette(palette);
}

void VID_Init(unsigned char *palette)
{
    vid.maxwarpwidth = vid.width = vid.conwidth = BASEWIDTH;
    vid.maxwarpheight = vid.height = vid.conheight = BASEHEIGHT;
    vid.aspect = 1.0;
    vid.numpages = 1;
    vid.colormap = host_colormap;
    vid.fullbright = 256 - LittleLong(*((int *)vid.colormap + 2048));
    vid.buffer = vid.conbuffer = vid_buffer;
    vid.rowbytes = vid.conrowbytes = BASEWIDTH;

    d_pzbuffer = zbuffer;
    D_InitCaches(surfcache, sizeof(surfcache));

    VID_SetPalette(palette);
    quake_enter_graphics();
}

void VID_Shutdown(void)
{
    quake_exit_graphics();
}

void VID_Update(vrect_t *rects)
{
    int i;
    (void)rects;

    for (i = 0; i < (BASEWIDTH * BASEHEIGHT); ++i)
    {
        unsigned int rgb = d_8to24table[vid_buffer[i] & 0xFF];
        rgba_buffer[i] = 0xFF000000u | rgb;
    }

    quake_draw_buffer(rgba_buffer, BASEWIDTH, BASEHEIGHT);
}

void D_BeginDirectRect(int x, int y, byte *pbitmap, int width, int height)
{
    (void)x;
    (void)y;
    (void)pbitmap;
    (void)width;
    (void)height;
}

void D_EndDirectRect(int x, int y, int width, int height)
{
    (void)x;
    (void)y;
    (void)width;
    (void)height;
}
