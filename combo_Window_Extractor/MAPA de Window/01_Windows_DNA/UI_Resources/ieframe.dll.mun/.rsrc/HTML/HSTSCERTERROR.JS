
function CertError()
{
error = '0';
DocQuery=document.location.search;
BeginError = DocQuery.indexOf("SSLError=");
if (BeginError > 0)
{
BeginError += 9;
EndError = DocQuery.indexOf("&", BeginError);
if (EndError > 0)
{
error = DocQuery.substring(BeginError,EndError);
}
else
{
error = DocQuery.substring(BeginError);
}
}
return error;
}
function closePage() {
window.close();
}
function BodyLoad()
{
var iError = CertError();
var sRealPageUrl = RealPageURL();
var iCertUnknownCA = 16777216;
var iCertExpired = 67108864;
var iCertCNMismatch = 33554432;
var iCertRevoked = 12170;
var iCertWeakSignature = 2097152;
if (iError == iCertRevoked)
{
document.getElementById("CertRevoked").style.display="block";
document.getElementById("CertRevoked").innerText = L_CertRevoked_TEXT;
document.getElementById("overridelink").style.display="none";
document.getElementById("ImgOverride").style.display="none";
}
else
{
if (iError & iCertUnknownCA)
{
document.getElementById("CertUnknownCA").style.display="block";
document.getElementById("CertUnknownCA").innerText = L_CertUnknownCA_TEXT;
}
if (iError & iCertExpired)
{
document.getElementById("CertExpired").style.display="block";
document.getElementById("CertExpired").innerText = L_CertExpired_TEXT;
}
if (iError & iCertCNMismatch)
{
document.getElementById("CertCNMismatch").style.display="block";
document.getElementById("CertCNMismatch").innerText = L_CertCNMismatch_TEXT;
}
if (iError & iCertWeakSignature)
{
document.getElementById("CertWeakSignature").style.display="block";
document.getElementById("CertWeakSignature").innerText = L_CertWeakSignature_TEXT;
}
if (iError == 0)
{
document.getElementById("CertSigFailed").style.display="block";
document.getElementById("CertSigFailed").innerText = L_CertSigFailed_TEXT;
}
}
removeNoScriptElements();
}
function RealPageURL()
{
urlresult = '';
DocURL=document.location.href;
BeginURL=DocURL.indexOf("#",1) + 1;
urlresult=DocURL.substring(BeginURL);
if (!isExternalUrlSafeForNavigation(urlresult))
{
urlresult = window.document.location.toString();
}
return urlresult;
}
