
var sRealPageUrl = RealPageURL();
var sMainTitle = L_TopFrameUnsafe_TEXT;
var sOriginalSite = "";
var sContentLocations = ContentLocations();
function RealPageURL()
{
urlresult = '';
DocURL = document.location.href;
BeginURL = DocURL.indexOf("#", 1) + 1;
urlresult = DocURL.substring(BeginURL);
if (!isExternalUrlSafeForNavigation(urlresult))
{
urlresult = window.document.location.toString();
}
return urlresult;
}
function closePage()
{
window.close();
}
function makeReportURL()
{
window.external.msReportSafeUrl();
}
function Threats()
{
threats = '0';
DocQuery = document.location.search;
BeginThreats = DocQuery.indexOf("Threats=");
if (BeginThreats > 0)
{
BeginThreats += 8;
EndThreats = DocQuery.indexOf("&", BeginThreats);
if (EndThreats > 0)
{
threats = DocQuery.substring(BeginThreats, EndThreats);
}
else
{
threats = DocQuery.substring(BeginThreats);
}
}
return threats;
}
function ContentLocations()
{
hostname = '0';
DocQuery = document.location.search;
BeginHostname = DocQuery.indexOf("Host=");
if (BeginHostname > 0)
{
BeginHostname += 5;
EndHostname = DocQuery.indexOf("&", BeginHostname);
if (EndHostname > 0)
{
hostname = DocQuery.substring(BeginHostname, EndHostname);
}
else
{
hostname = DocQuery.substring(BeginHostname);
}
}
if (DocQuery.indexOf("BlockedDomain=") > 0)
{
sMainTitle = L_ContentUnsafe_TEXT;
sOriginalSite = L_Content_TEXT + hostname;
return BlockedDomain();
}
return hostname;
}
function BlockedDomain()
{
blockedName = "0";
BeginBlockedName = DocQuery.indexOf("BlockedDomain=");
if (BeginBlockedName > 0)
{
BeginBlockedName += 14;
EndBlockedName = DocQuery.indexOf("&", BeginBlockedName);
if (EndBlockedName > 0)
{
blockedName = DocQuery.substring(BeginBlockedName, EndBlockedName);
}
else
{
blockedName = DocQuery.substring(BeginBlockedName);
}
}
return blockedName;
}
function BlockOverride()
{
blockOverride = '0';
DocQuery = document.location.search;
BeginBlock = DocQuery.indexOf("Block=");
if (BeginBlock > 0)
{
BeginBlock += 6;
EndBlock = DocQuery.indexOf("&", BeginBlock);
if (EndBlock > 0)
{
blockOverride = DocQuery.substring(BeginBlock, EndBlock);
}
else
{
blockOverride = DocQuery.substring(BeginBlock);
}
}
return blockOverride;
}
function BodyLoad()
{
var iThreats = Threats();
var iPhishing = 4;
var iMalware = 16;
var iUnwanted = 512;
var overridden = false;
if (iThreats & iPhishing)
{
document.getElementById("PhishingThreat").style.display = "list-item";
document.getElementById("PhishingThreat").innerText = L_PhishingThreat_TEXT;
overridden = true;
}
if (iThreats & iMalware)
{
document.getElementById("MalwareThreat").style.display = "list-item";
document.getElementById("MalwareThreat").innerText = L_MalwareThreat_TEXT;
overridden = true;
}
if (!overridden && (iThreats & iUnwanted)){
sMainTitle = L_TopFrameUnwanted_TEXT;
document.title = L_PUAATitle_TEXT;
document.getElementById("recommendation").style.display = "none";
document.getElementById("puaaRecommendation").style.display = "block";
document.getElementById("moreInfoDropdown").style.display = "none";
document.getElementById("actionsListPuaa").style.display = "block";
document.getElementById("errorIcon").style.display = "none";
document.getElementById("homepageButton").style.display = "none";
document.getElementById("background").setAttribute("class", "black");
document.getElementById("msgBackground").setAttribute("class", "msgBlock black");
document.getElementById("msgBody").setAttribute("class", "black");
document.getElementById("mainTitle").setAttribute("class", "redFont");
document.getElementById("contentlocations").style.color = "#575757";
document.getElementById("hostsite").style.color = "#575757";
document.getElementById("puaaRecommendation").style.color = "#575757";
}
var iBlockOverride = BlockOverride();
if (iBlockOverride == 0)
{
document.getElementById("DoOverride").style.display = "block";
document.getElementById("overrideLink").href = sRealPageUrl;
if (!overridden && (iThreats & iUnwanted)){
document.getElementById("DoOverridePuaa").style.display = "block";
document.getElementById("overrideLinkPuaa").href = sRealPageUrl;
}
}
else
{
document.getElementById("DoOverride").style.display = "none";
if (!overridden && (iThreats & iUnwanted)){
document.getElementById("DoOverridePuaa").style.display = "none";
}
}
document.getElementById("contentlocations").innerText = sContentLocations;
if (sOriginalSite)
{
document.getElementById("hostsite").innerText = sOriginalSite;
}
else
{
document.getElementById("hostsite").style.visibility = "hidden";
}
document.getElementById("mainTitle").innerText = sMainTitle;
removeNoScriptElements();
ReverseForRTL(iThreats & iUnwanted);
document.getElementById("moreInformationDropdownSpan").innerText = L_MOREINFO_TEXT;
var icons = document.getElementsByClassName("msgIcon white");
for (var i = 0; i < icons.length; i++) {
icons[i].setAttribute("aria-hidden", "true");
}
}
function ReverseForRTL(isPua)
{
if (document.dir == "rtl")
{
document.getElementById("threatsList").className = "ul-rtl";
document.getElementById("PhishingThreat").className = "bullet-li-rtl";
document.getElementById("MalwareThreat").className = "bullet-li-rtl";
document.getElementById("actionsList").className = "ul-rtl";
document.getElementById("DoReportSafe").className = "caret-li-rtl";
document.getElementById("DoOverride").className = "caret-li-rtl";
if (isPua){
document.getElementById("actionsListPuaa").className = "ul-rtl";
document.getElementById("DoReportSafePuaa").className = "caret-li-rtl";
document.getElementById("DoOverridePuaa").className = "caret-li-rtl";
}
document.getElementById("branding").style.textAlign = "left";
}
}
function expandCollapseIM(elem)
{
if (document.getElementById)
{
ecBlock = document.getElementById(elem);
if (ecBlock !== undefined && ecBlock !== null)
{
if (ecBlock.currentStyle.display === "block")
{
ecBlock.style.display = "none";
}
else
{
ecBlock.style.display = "block";
}
}
}
}
