// Windows Media Player - Copyright 2000 Microsoft Corporation.
//<script>

// Dimensions must match globals.cpp

var g_BannerHeight		= 36;
var g_SepWidth			= 3;
var g_SepHeight 		= 3;

var g_SettingsMinHeight = 21;
var g_SettingsMaxHeight = 500;
var g_SidebarMinWidth	= 100;
var g_SidebarMaxWidth	= 500;
var g_PlaylistMinHeight = 35;
var g_MetadataMinHeight = 26;

var g_DisplayMinWidth	= 200;
var g_DisplayMinHeight	= 39;
var g_MediaMinHeight	= 20;

var PLUGIN_TYPE_BACKGROUND				=		1;
var PLUGIN_TYPE_SEPARATEWINDOW			=		2;
var PLUGIN_TYPE_DISPLAYAREA 			=		3;
var PLUGIN_TYPE_SETTINGSAREA			=		4;
var PLUGIN_TYPE_METADATAAREA			=		5;
var PLUGIN_TYPE_BANNERAREA				=		240;

var PLUGIN_PROP_CURRENTPRESET			= "CurrentPreset"
var PLUGIN_PROP_SKINFILE				= "SkinFile"

var VIEW_FLAGSEX_ATTEMPTEDAUTORUN		=		0x0001;

var g_HResize = false;

var g_fCaptionsDetected = false;
var g_fShowCaptions = false;
var g_fShowBanner	= false;
var g_fClosing = false;

var g_rgSettings = new Array(
	"CrossFadeView",
	"EQView",
	"AdvancedVideoSettingsView",
	"AdvancedAudioView",
	"SRSView",
	"VideoSettingsView",
	"DolbyDigitalView"
	);

var g_nSettingsTitleLeft = 0;

function OnLoad()
{
	g_fCaptionsDetected = helper.captionsAvailable;


	ResetDisplayPlugin();
	UpdateCaptions();
	UpdateBannerPlugin();
	UpdateSettingsPlugin();
	UpdatePanels();
}

function OnOpenStateChange()
{
	if( osMediaOpen == player.OpenState )
	{
		g_fCaptionsDetected = helper.captionsAvailable;
		ResetDisplayPlugin();
		UpdateCaptions();
		UpdateBannerPlugin();
		UpdatePanels();
		
	}

}

function OnPlayStateChange()
{
	if( psPlaying == player.playState )
	{
		ResetDisplayPlugin();
		UpdateCaptions();
		UpdatePanels();	
	}
}

function OnScriptCommand(scType, Param)
{
	scType = scType.toUpperCase();
	
	if(scType == "TEXT" ||	scType == "<SYNC" || scType == "CAPTION" || scType == "SYNCHRONIZEDLYRICLYRIC")
	{
		g_fCaptionsDetected = true;
		UpdateCaptions();
		UpdatePanels();
	}
}

function UpdatePanels()
{
   if( !g_fClosing )
   {
	var lSettings = 0;
	var lSidebar = 0;
	var lMetadata = 0;
	var lPlaylist = 0;
	var lDisplay = 0;
	var lCaptions = 0;
	var lBanner = 0;

	helper.calcLayout( vMain.width, vMain.height, g_fShowCaptions, g_fShowBanner );

	lSettings = helper.getLayoutSize( 1 );
	lSidebar = helper.getLayoutSize( 2 );
	lMetadata = helper.getLayoutSize( 3 );
	lPlaylist = helper.getLayoutSize( 4 );
	lDisplay = helper.getLayoutSize( 5 );
	lCaptions = helper.getLayoutSize( 6 );
	lBanner = helper.getLayoutSize( 7 );

    svMedia.height = svMain.height-svMedia.top;
    
    svMain.width = vMain.width;
		
	UpdateResizeBars();

	if( 0 != lDisplay )
	{
		piDisplay.height = lDisplay;
		piDisplay.visible = true;
	}
	else
	{
		piDisplay.visible = false;
	}

	piBanner.visible = ( 0 != lBanner );

	if( 0 != lCaptions )
	{
		piCaptions.verticalAlignment = "top";
		piCaptions.top = lDisplay;
		piCaptions.height = lCaptions;
		piCaptions.verticalAlignment = "bottom";
		piCaptions.visible = true;
	}
	else
	{
		piCaptions.visible = false;
	}
   }
}

function ResetDisplayPlugin()
{
		mediacenter.displayView = helper.resolveDisplayView(false);
		UpdateDisplayPlugin();
		UpdateDisplayPreset();
}

var g_fSetQuickHideForBorder=false;

function OnClose()
{
   g_fClosing = true;
   if( g_fSetQuickHideForBorder )
   {
      QuickHideForBorder( false );
   }
}

function QuickHideForBorder(fHide)
{
	if (fHide && (!mediacenter.quickHide) && (!g_fSetQuickHideForBorder))
	{
		g_fSetQuickHideForBorder = true;
		mediacenter.quickHide=true;
	}
	else if ( (!fHide) && g_fSetQuickHideForBorder)
	{
		mediacenter.quickHide=false;
		g_fSetQuickHideForBorder=false;
	}
}

function UpdateDisplayPlugin()
{
	piDisplay.objectID = mediacenter.displayView;

	// handle skin

	if( piDisplay.objectID == "BorderView" )
	{
		QuickHideForBorder(true);		
		var skinfile = helper.getSkinFile();
		piDisplay.setProperty( PLUGIN_PROP_SKINFILE, skinfile );

		// Reset if error

		if( ( piDisplay.getProperty( PLUGIN_PROP_SKINFILE ) != skinfile ) || ( "" == skinfile ) )
		{
			mediacenter.displayView = helper.resolveDisplayView(true);
			UpdateDisplayPreset();
			QuickHideForBorder(false);
		}
	}
	else
	{		
		QuickHideForBorder(false);		
	}

	// Clear the security goo for anything that isn't a border view or metadata view
	if( piDisplay.objectID != "BorderView" && piDisplay.objectID != "MetadataView" )
	{
		helper.htmlViewBaseURL = "";
		helper.htmlViewFullURL = "";
		helper.htmlViewSecureLock = 0;
		helper.htmlViewBusy = false;
	}



	// Reset if error

	if( "" == piDisplay.objectID )
	{
			mediacenter.displayView = helper.resolveDisplayView(true);
			UpdateDisplayPreset();
	}

	// Reset full screen

	if( piDisplay.objectID != "VideoView" && piDisplay.objectID != "VizView" )
	{
			mediacenter.fullScreenAvailable = false;
	}

	// Sync back

	mediacenter.displayView = piDisplay.objectID;

}

function UpdateDisplayPreset()
{
	var displayPreset = 0;

	if( piDisplay.objectID != "VideoView" && piDisplay.objectID != "VizView" && piDisplay.objectID != "BorderView" && piDisplay.objectID != "MetadataView" )
	{
		piDisplay.setProperty( PLUGIN_PROP_CURRENTPRESET, mediacenter.displayPreset );

		// Sync back

		displayPreset = piDisplay.getProperty( PLUGIN_PROP_CURRENTPRESET );
		mediacenter.displayPreset = displayPreset;
	}
}

function UpdateCaptions()
{
	var fShowCaptions = false;
	
	if ( mediacenter.showCaptions && !PlayingDVD() )
	{
		if( piDisplay.objectID == "VideoView" || piDisplay.objectID == "VizView" || piDisplay.objectID == "AlbumArtView" )
		{		
			fShowCaptions = g_fCaptionsDetected;
		}
	}
	
	g_fShowCaptions = fShowCaptions;
}

function UpdateBannerPlugin()
{
	if(player.currentMedia && (player.currentmedia.getItemInfo("BannerURL") != ""))
	{
		g_fShowBanner = true;
	}
	else
	{
		g_fShowBanner = false;
	}
}

function UpdateSettingsPlugin()
{
    return;
}


function PreviousDisplay()
{
	if( "VizView" == piDisplay.objectID )
	{
		if (event.shiftKey)
		{
			mediacenter.vizRequest = VR_VIZ_PREV;
		}
		else
		{
			mediacenter.vizRequest = VR_PRESET_PREV;
		}
	}
	else
	{
		var presetCount = helper.viewPresetCount( piDisplay.objectID );
		var preset = mediacenter.displayPreset;

		preset = preset - 1;

		if( preset < 0 )
		{
			preset = presetCount - 1;
		}

		mediacenter.displayPreset = preset;
	}
}

function NextDisplay()
{
	if( "VizView" == piDisplay.objectID )
	{
		if (event.shiftKey)
		{
			mediacenter.vizRequest = VR_VIZ_NEXT;
		}
		else
		{
			mediacenter.vizRequest = VR_PRESET_NEXT;
		}
	}
	else
	{
		var presetCount = helper.viewPresetCount( piDisplay.objectID );
		var preset = mediacenter.displayPreset;

		preset = preset + 1;

		if( preset >= presetCount )
		{
			preset = 0;
		}

		mediacenter.displayPreset = preset;
	}
}

function PreviousSettings()
{
	var nIndex = MapSettingsToIndex( mediacenter.settingsView );
	var nCurrentIndex = nIndex;
	
	do
	{
		nIndex = nIndex - 1;

		if( 0 > nIndex )
		{
			nIndex = g_rgSettings.length - 1;
		}

		mediacenter.settingsView = g_rgSettings[ nIndex ];

	} while( ( MapSettingsToIndex( mediacenter.settingsView ) == nCurrentIndex ) && ( nCurrentIndex != nIndex ) );
}

function NextSettings()
{
	var nIndex = MapSettingsToIndex( mediacenter.settingsView );
	var nCurrentIndex = nIndex;
	
	do
	{
		nIndex = nIndex + 1;

		if( g_rgSettings.length <= nIndex )
		{
			nIndex = 0;
		}

		mediacenter.settingsView = g_rgSettings[ nIndex ];

	} while( ( MapSettingsToIndex( mediacenter.settingsView ) == nCurrentIndex ) && ( nCurrentIndex != nIndex ) );
}

function UpdateResizeBars()
{
    btnHResize.visible = false;
}

function OnLinkRequest()
{
	theme.CurrentViewID="vBrowser";
}

function OnBrowserLoad()
{
	Browser1.SetProperty( "linkRequestParams", helper.linkRequestParams );
	Browser1.SetProperty( "linkRequest", helper.linkRequest );
}

function OnBrowserBack()
{
	Browser1.visible=false;
	vBrowser.updateWindow();
	theme.CurrentViewID="vMain";
}

function PlayingDVD()
{
	return ( (player.currentPlaylist.count>0) && (player.currentPlaylist.item(0).sourceURL.indexOf("wmpdvd:")==0));
}

function MapSettingsToIndex( View )
{
	for (var i = 0; i < g_rgSettings.length; i++)
	{
		if (View == g_rgSettings[i])
		{
			return( i );
		}
	}

	return( -1 );
}

