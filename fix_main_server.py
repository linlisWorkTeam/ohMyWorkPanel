import sys
with open(sys.argv[1], 'r') as f:
    c = f.read()
c = c.replace('.nest_service("/", ServeDir::new(&dist_dir))', '.fallback_service(ServeDir::new(&dist_dir))')
with open(sys.argv[1], 'w') as f:
    f.write(c)
print('Fixed:', sys.argv[1])
