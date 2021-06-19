set -ex
cd $(dirname $0)
rm -rf ./_zipped.zip
rm -rf ./_unzipped
zip ./_zipped.zip ./zip.txt
unzip -u ./_zipped.zip -d ./_unzipped
cmp ./_unzipped/zip.txt ./zip.txt