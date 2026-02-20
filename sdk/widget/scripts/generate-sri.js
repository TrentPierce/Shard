const fs = require('fs');
const crypto = require('crypto');
const path = require('path');

try {
    const filePath = path.join(__dirname, '../dist/scout.js');
    if (!fs.existsSync(filePath)) {
        console.error("scout.js not found in dist/. Did the build fail?");
        process.exit(1);
    }
    const fileBuffer = fs.readFileSync(filePath);
    const hashSum = crypto.createHash('sha384');
    hashSum.update(fileBuffer);
    const sriBase64 = hashSum.digest('base64');
    const sriString = `sha384-${sriBase64}`;

    console.log(`\n===========================================`);
    console.log(`✅ Build Complete: scout.js`);
    console.log(`🔒 Subresource Integrity (SRI) Hash:`);
    console.log(`   ${sriString}`);
    console.log(`===========================================\n`);

    fs.writeFileSync(path.join(__dirname, '../dist/sri.txt'), sriString);
} catch (error) {
    console.error("Error generating SRI hash:", error);
    process.exit(1);
}
