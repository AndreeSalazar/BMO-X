// Dimensions must match globals.cpp
var g_SettingsMinHeight = 21;
var g_SettingsMaxHeight = 500;

var PLUGIN_PROP_CURRENTPRESET			= "CurrentPreset"

var g_rgSettings = new Array(
	"AppColorView",
	"CrossFadeView",
	"EQView",
	"MediaLinkView",
	"AdvancedVideoSettingsView",
	"AdvancedAudioView",
	"SRSView",
	"VideoSettingsView",
	"DolbyDigitalView"
	);

var g_nSettingsTitleLeft = 0;

function OnLoad()
{
	g_nSettingsTitleLeft = txtSettingsTitle.left;
}

function UpdateSettingsPlugin()
{
	var settingsPreset = 0;

	piSettings.objectID = mediacenter.settingsView;
	piSettings.setProperty( PLUGIN_PROP_CURRENTPRESET, mediacenter.settingsPreset );

	// Reset if error

	if( "" == piSettings.objectID )
	{
			piSettings.objectID = "EQView";
	}

	// Sync back

	settingsPreset = piSettings.getProperty( PLUGIN_PROP_CURRENTPRESET );
	mediacenter.settingsView = piSettings.objectID;
	mediacenter.settingsPreset = settingsPreset;

	// Update spinners and text

	if( 0 <= MapSettingsToIndex( mediacenter.settingsView ) )
	{
		btnSettingsPrev.visible = true;
		txtSettingsTitle.horizontalAlignment="left";
		txtSettingsTitle.left = g_nSettingsTitleLeft;
		txtSettingsTitle.width = btnSettingsClose.left-txtSettingsTitle.left;
		txtSettingsTitle.horizontalAlignment="stretch";
	}
	else
	{
		btnSettingsPrev.visible = false;
		txtSettingsTitle.horizontalAlignment="left";
		txtSettingsTitle.left = btnSettingsPrev.left;
		txtSettingsTitle.width = btnSettingsClose.left-txtSettingsTitle.left;
		txtSettingsTitle.horizontalAlignment="stretch";
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

function UpdateSettingsTitle()
{
		var viewName = helper.viewFriendlyName( piSettings.objectID );

		txtSettingsTitle.value = viewName;
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

