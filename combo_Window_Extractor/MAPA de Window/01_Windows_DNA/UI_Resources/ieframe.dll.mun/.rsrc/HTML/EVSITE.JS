
var sVendorName = VendorName();
var sHost = HostName();
var fHaveBlockedDomain = false;
var sBlockedDomain = BlockedDomain();
function closePage()
{
window.close();
}
function BlockedDomain() {
var blockedName = "0";
var DocQuery = document.location.search;
var BeginBlockedName = DocQuery.indexOf("BlockedDomain=");
if (BeginBlockedName > 0) {
BeginBlockedName += 14;
var EndBlockedName = DocQuery.indexOf("&", BeginBlockedName);
if (EndBlockedName > 0) {
blockedName = DocQuery.substring(BeginBlockedName, EndBlockedName);
}
else {
blockedName = DocQuery.substring(BeginBlockedName);
}
fHaveBlockedDomain = true;
}
return blockedName;
}
function VendorName() {
var vendorname = '0';
var DocQuery = document.location.search;
var BeginVendorname = DocQuery.indexOf("Vendor=");
if (BeginVendorname > 0) {
BeginVendorname += 7;
var EndVendorname = DocQuery.indexOf("&", BeginVendorname);
if (EndVendorname > 0) {
vendorname = DocQuery.substring(BeginVendorname, EndVendorname);
}
else {
vendorname = DocQuery.substring(BeginVendorname);
}
}
return vendorname;
}
function HostName() {
var hostname = '0';
var DocQuery = document.location.search;
var BeginHostname = DocQuery.indexOf("Host=");
if (BeginHostname > 0) {
BeginHostname += 5;
var EndHostname = DocQuery.indexOf("&", BeginHostname);
if (EndHostname > 0) {
hostname = DocQuery.substring(BeginHostname, EndHostname);
}
else {
hostname = DocQuery.substring(BeginHostname);
}
}
return hostname;
}
function InitTitle()
{
var bTitle = document.getElementById("mainTitle");
if (fHaveBlockedDomain)
{
var re = /%s/g;
bTitle.innerText = L_ActiveXUnsafe_Frame_TEXT.replace(re, sVendorName);
}
else
{
var re = /%s/g;
bTitle.innerText = L_ActiveXUnsafe_TEXT.replace(re, sVendorName);
}
}
function InitContentLocationAndHostSite()
{
var bContentLocation = document.getElementById("contentlocations");
var bHostSite = document.getElementById("hostsite");
if (fHaveBlockedDomain)
{
bContentLocation.innerText = sBlockedDomain;
var re = /%s/g;
bHostSite.innerText = L_ActiveXUnsafe_Hosted_TEXT.replace(re, sHost);
}
else
{
bContentLocation.innerText = sHost;
}
}
function InitEvExplain()
{
var re = /%s/g;
document.getElementById("evExplain").innerText = L_ActiveXUnsafe_Explain_TEXT.replace(re, sVendorName);
}
function BodyLoad()
{
InitTitle();
InitContentLocationAndHostSite();
InitEvExplain();
removeNoScriptElements();
ReverseCornersForRTL();
}
function ReverseCornersForRTL()
{
if (document.dir == "rtl")
{
document.getElementById("top_leading_corner").src = "ne.gif";
document.getElementById("top_trailing_corner").src = "nw.gif";
document.getElementById("bottom_leading_corner").src = "se.gif";
document.getElementById("bottom_trailing_corner").src = "sw.gif";
document.getElementById("mainTitleAlign").align = "right";
document.getElementById("evAlign").align = "right";
document.getElementById("whatIsPF2Align").align = "right";
}
}
