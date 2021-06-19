set -ex
cd $(dirname $0)
rm -rf ./_sed.out
sed 's/\([A-Za-z]\)/\(\1\)/g' < ./sed.in > ./_sed.out
cmp ./_sed.out ./sed.out