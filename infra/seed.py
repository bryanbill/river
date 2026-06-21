#!/usr/bin/env python3
"""
River Database Seed Script
Generates 10,000 related rows per table across Postgres, MySQL, SQLite, and MongoDB.

Tables:
  - users       (10,000) — id, name, email, department, salary, status, is_verified, created_at
  - products    (10,000) — id, name, category, price, stock, rating, is_active, created_at
  - orders      (10,000) — id, user_id→users, status, total, created_at
  - order_items (10,000) — id, order_id→orders, product_id→products, quantity, unit_price

Usage: python3 infra/seed.py
"""

import subprocess
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).parent
PROJECT_DIR = SCRIPT_DIR.parent

ROWS = 10_000
BATCH = 500

FIRST_NAMES = [
    "Alice", "Bob", "Carol", "Dave", "Eve", "Frank", "Grace", "Henry",
    "Iris", "Jack", "Kate", "Liam", "Mia", "Noah", "Olivia", "Paul",
    "Quinn", "Rose", "Sam", "Tina", "Uma", "Vince", "Wendy", "Xander",
    "Yuki", "Zara"
]
LAST_NAMES = [
    "Smith", "Johnson", "Williams", "Brown", "Jones", "Garcia", "Miller",
    "Davis", "Rodriguez", "Martinez", "Hernandez", "Lopez", "Wilson",
    "Anderson", "Thomas", "Taylor", "Moore", "Jackson", "Martin", "Lee",
    "Perez", "Thompson", "White", "Harris", "Sanchez", "Clark"
]
DEPARTMENTS = [
    "Engineering", "Sales", "Marketing", "Support", "Finance",
    "HR", "Legal", "Product", "Design", "Operations"
]
USER_STATUSES = ["active", "inactive", "suspended", "pending"]
CATEGORIES = [
    "Electronics", "Clothing", "Books", "Home", "Sports",
    "Food", "Toys", "Health", "Automotive", "Garden"
]
PRODUCT_ADJ = [
    "Premium", "Basic", "Pro", "Ultra", "Mini",
    "Super", "Mega", "Elite", "Nano", "Max"
]
PRODUCT_NOUN = [
    "Widget", "Gadget", "Device", "Tool", "Kit",
    "Pack", "Set", "System", "Module", "Unit"
]
ORDER_STATUSES = ["pending", "paid", "shipped", "delivered", "cancelled", "refunded"]


def user_row(i):
    fn = FIRST_NAMES[(i * 7 + 3) % len(FIRST_NAMES)]
    ln = LAST_NAMES[(i * 13 + 5) % len(LAST_NAMES)]
    name = f"{fn} {ln}"
    email = f"{fn.lower()}.{ln.lower()}{i}@example.com"
    dept = DEPARTMENTS[(i * 11 + 2) % len(DEPARTMENTS)]
    salary = 35000 + (i * 17) % 115000
    status = USER_STATUSES[(i * 3) % len(USER_STATUSES)]
    is_verified = i % 3 == 0
    return name, email, dept, salary, status, is_verified


def product_row(i):
    adj = PRODUCT_ADJ[(i * 7) % len(PRODUCT_ADJ)]
    noun = PRODUCT_NOUN[(i * 3) % len(PRODUCT_NOUN)]
    name = f"{adj} {noun} {i}"
    category = CATEGORIES[(i * 11) % len(CATEGORIES)]
    price = round(0.99 + (i * 31) % 99901 / 100, 2)
    stock = (i * 7) % 500
    rating = round(1.0 + (i * 13) % 400 / 100, 2)
    is_active = i % 10 != 0
    return name, category, price, stock, rating, is_active


def order_row(i):
    user_id = (i * 7 + 1) % ROWS + 1
    status = ORDER_STATUSES[(i * 5) % len(ORDER_STATUSES)]
    total = round(5.0 + (i * 43) % 50000 / 100, 2)
    return user_id, status, total


def order_item_row(i):
    order_id = (i * 3 + 1) % ROWS + 1
    product_id = (i * 11 + 2) % ROWS + 1
    quantity = 1 + (i * 7) % 10
    unit_price = round(0.99 + (i * 31) % 99901 / 100, 2)
    return order_id, product_id, quantity, unit_price


def escape_sql(s):
    return s.replace("'", "''")


def generate_sql(dialect):
    """Generate SQL for postgres, mysql, or sqlite."""
    lines = []

    if dialect == "postgres":
        auto_inc = "SERIAL PRIMARY KEY"
        bool_true, bool_false = "TRUE", "FALSE"
    elif dialect == "mysql":
        auto_inc = "INT AUTO_INCREMENT PRIMARY KEY"
        bool_true, bool_false = "TRUE", "FALSE"
    else:  # sqlite
        auto_inc = "INTEGER PRIMARY KEY AUTOINCREMENT"
        bool_true, bool_false = "1", "0"

    if dialect == "mysql":
        name_type = "VARCHAR(200)"
        status_type = "VARCHAR(50)"
    else:
        name_type = "TEXT"
        status_type = "TEXT"

    lines.append("DROP TABLE IF EXISTS order_items;")
    lines.append("DROP TABLE IF EXISTS orders;")
    lines.append("DROP TABLE IF EXISTS products;")
    lines.append("DROP TABLE IF EXISTS users;")
    lines.append("")
    lines.append(f"""CREATE TABLE users (
    id {auto_inc},
    name {name_type} NOT NULL,
    email {name_type} NOT NULL,
    department {status_type},
    salary DECIMAL(10,2),
    status {status_type} DEFAULT 'active',
    is_verified BOOLEAN DEFAULT {bool_false},
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);""")
    lines.append("")
    lines.append(f"""CREATE TABLE products (
    id {auto_inc},
    name {name_type} NOT NULL,
    category {status_type} NOT NULL,
    price DECIMAL(10,2) NOT NULL,
    stock INT DEFAULT 0,
    rating DECIMAL(3,2),
    is_active BOOLEAN DEFAULT {bool_true},
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);""")
    lines.append("")
    lines.append(f"""CREATE TABLE orders (
    id {auto_inc},
    user_id INT NOT NULL,
    status {status_type} DEFAULT 'pending',
    total DECIMAL(10,2) DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);""")
    lines.append("")
    lines.append(f"""CREATE TABLE order_items (
    id {auto_inc},
    order_id INT NOT NULL,
    product_id INT NOT NULL,
    quantity INT NOT NULL DEFAULT 1,
    unit_price DECIMAL(10,2) NOT NULL
);""")
    lines.append("")

    # Users
    for batch_start in range(1, ROWS + 1, BATCH):
        batch_end = min(batch_start + BATCH - 1, ROWS)
        values = []
        for i in range(batch_start, batch_end + 1):
            name, email, dept, salary, status, verified = user_row(i)
            bv = bool_true if verified else bool_false
            values.append(
                f"('{escape_sql(name)}', '{escape_sql(email)}', '{dept}', {salary}, '{status}', {bv})"
            )
        lines.append(
            f"INSERT INTO users (name, email, department, salary, status, is_verified) VALUES\n"
            + ",\n".join(values) + ";"
        )

    lines.append("")

    # Products
    for batch_start in range(1, ROWS + 1, BATCH):
        batch_end = min(batch_start + BATCH - 1, ROWS)
        values = []
        for i in range(batch_start, batch_end + 1):
            name, category, price, stock, rating, is_active = product_row(i)
            av = bool_true if is_active else bool_false
            values.append(
                f"('{escape_sql(name)}', '{category}', {price}, {stock}, {rating}, {av})"
            )
        lines.append(
            f"INSERT INTO products (name, category, price, stock, rating, is_active) VALUES\n"
            + ",\n".join(values) + ";"
        )

    lines.append("")

    # Orders
    for batch_start in range(1, ROWS + 1, BATCH):
        batch_end = min(batch_start + BATCH - 1, ROWS)
        values = []
        for i in range(batch_start, batch_end + 1):
            user_id, status, total = order_row(i)
            values.append(f"({user_id}, '{status}', {total})")
        lines.append(
            f"INSERT INTO orders (user_id, status, total) VALUES\n"
            + ",\n".join(values) + ";"
        )

    lines.append("")

    # Order Items
    for batch_start in range(1, ROWS + 1, BATCH):
        batch_end = min(batch_start + BATCH - 1, ROWS)
        values = []
        for i in range(batch_start, batch_end + 1):
            order_id, product_id, quantity, unit_price = order_item_row(i)
            values.append(f"({order_id}, {product_id}, {quantity}, {unit_price})")
        lines.append(
            f"INSERT INTO order_items (order_id, product_id, quantity, unit_price) VALUES\n"
            + ",\n".join(values) + ";"
        )

    # Indexes
    lines.append("")
    lines.append("CREATE INDEX idx_users_department ON users(department);")
    lines.append("CREATE INDEX idx_users_status ON users(status);")
    lines.append("CREATE INDEX idx_orders_user_id ON orders(user_id);")
    lines.append("CREATE INDEX idx_orders_status ON orders(status);")
    lines.append("CREATE INDEX idx_order_items_order_id ON order_items(order_id);")
    lines.append("CREATE INDEX idx_order_items_product_id ON order_items(product_id);")
    lines.append("CREATE INDEX idx_products_category ON products(category);")

    return "\n".join(lines)


def generate_mongo_js():
    """Generate compact mongosh JavaScript that generates data in loops."""
    return '''
db = db.getSiblingDB("river");
db.users.drop();
db.products.drop();
db.orders.drop();
db.order_items.drop();

const fn = ["Alice","Bob","Carol","Dave","Eve","Frank","Grace","Henry","Iris","Jack","Kate","Liam","Mia","Noah","Olivia","Paul","Quinn","Rose","Sam","Tina","Uma","Vince","Wendy","Xander","Yuki","Zara"];
const ln = ["Smith","Johnson","Williams","Brown","Jones","Garcia","Miller","Davis","Rodriguez","Martinez","Hernandez","Lopez","Wilson","Anderson","Thomas","Taylor","Moore","Jackson","Martin","Lee","Perez","Thompson","White","Harris","Sanchez","Clark"];
const depts = ["Engineering","Sales","Marketing","Support","Finance","HR","Legal","Product","Design","Operations"];
const ustat = ["active","inactive","suspended","pending"];
const cats = ["Electronics","Clothing","Books","Home","Sports","Food","Toys","Health","Automotive","Garden"];
const padj = ["Premium","Basic","Pro","Ultra","Mini","Super","Mega","Elite","Nano","Max"];
const pnoun = ["Widget","Gadget","Device","Tool","Kit","Pack","Set","System","Module","Unit"];
const ostat = ["pending","paid","shipped","delivered","cancelled","refunded"];

print("Seeding users...");
let b = [];
for (let i = 1; i <= 10000; i++) {
    const f = fn[(i*7+3)%fn.length], l = ln[(i*13+5)%ln.length];
    b.push({_id:i, name:f+" "+l, email:f.toLowerCase()+"."+l.toLowerCase()+i+"@example.com",
        department:depts[(i*11+2)%depts.length], salary:35000+(i*17)%115000,
        status:ustat[(i*3)%ustat.length], is_verified:i%3===0,
        created_at:new Date(2023,0,1+i%365,i%24,i%60)});
    if (b.length===1000) { db.users.insertMany(b); b=[]; }
}
if (b.length>0) db.users.insertMany(b);
print("  users: "+db.users.countDocuments());

print("Seeding products...");
b = [];
for (let i = 1; i <= 10000; i++) {
    b.push({_id:i, name:padj[(i*7)%padj.length]+" "+pnoun[(i*3)%pnoun.length]+" "+i,
        category:cats[(i*11)%cats.length],
        price:Math.round((0.99+(i*31)%99901/100)*100)/100,
        stock:(i*7)%500, rating:Math.round((1+(i*13)%400/100)*100)/100,
        is_active:i%10!==0, created_at:new Date(2023,0,1+i%365)});
    if (b.length===1000) { db.products.insertMany(b); b=[]; }
}
if (b.length>0) db.products.insertMany(b);
print("  products: "+db.products.countDocuments());

print("Seeding orders...");
b = [];
for (let i = 1; i <= 10000; i++) {
    b.push({_id:i, user_id:(i*7+1)%10000+1, status:ostat[(i*5)%ostat.length],
        total:Math.round((5+(i*43)%50000/100)*100)/100,
        created_at:new Date(2024,0,1+i%365,i%24,i%60)});
    if (b.length===1000) { db.orders.insertMany(b); b=[]; }
}
if (b.length>0) db.orders.insertMany(b);
print("  orders: "+db.orders.countDocuments());

print("Seeding order_items...");
b = [];
for (let i = 1; i <= 10000; i++) {
    b.push({_id:i, order_id:(i*3+1)%10000+1, product_id:(i*11+2)%10000+1,
        quantity:1+(i*7)%10, unit_price:Math.round((0.99+(i*31)%99901/100)*100)/100});
    if (b.length===1000) { db.order_items.insertMany(b); b=[]; }
}
if (b.length>0) db.order_items.insertMany(b);
print("  order_items: "+db.order_items.countDocuments());

db.users.createIndex({email:1},{unique:true});
db.users.createIndex({department:1});
db.users.createIndex({status:1});
db.orders.createIndex({user_id:1});
db.orders.createIndex({status:1});
db.order_items.createIndex({order_id:1});
db.order_items.createIndex({product_id:1});
db.products.createIndex({category:1});
print("Done.");
'''


def run(cmd, input_data=None, label="", timeout=120):
    result = subprocess.run(
        cmd, input=input_data, capture_output=True, text=True, timeout=timeout
    )
    if result.returncode != 0:
        stderr = result.stderr.strip()
        # Filter MySQL password warning
        stderr_lines = [l for l in stderr.split('\n') if 'Using a password' not in l]
        err_msg = '\n'.join(stderr_lines).strip()
        if err_msg:
            print(f"  ERROR ({label}): {err_msg[:300]}")
            return False
    return True


def main():
    print("=== River Database Seed Script ===")
    print(f"Generating {ROWS:,} rows per table across 4 databases\n")

    # 1. PostgreSQL
    print("[1/4] Seeding PostgreSQL...")
    pg_sql = generate_sql("postgres")
    ok = run(
        ["docker", "exec", "-i", "infra-postgres-1", "psql", "-U", "river", "-d", "river", "-q"],
        input_data=pg_sql,
        label="postgres"
    )
    if ok:
        print("  ✓ PostgreSQL done")

    # 2. MySQL
    print("[2/4] Seeding MySQL...")
    mysql_sql = generate_sql("mysql")
    ok = run(
        ["docker", "exec", "-i", "infra-mysql-1", "mysql", "-uriver", "-priver", "river"],
        input_data=mysql_sql,
        label="mysql"
    )
    if ok:
        print("  ✓ MySQL done")

    # 3. SQLite
    print("[3/4] Seeding SQLite...")
    sqlite_db = PROJECT_DIR / "river.db"
    sqlite_db.unlink(missing_ok=True)
    sqlite_sql = generate_sql("sqlite")
    ok = run(
        ["sqlite3", str(sqlite_db)],
        input_data=sqlite_sql,
        label="sqlite"
    )
    if ok:
        print(f"  ✓ SQLite done ({sqlite_db})")

    # 4. MongoDB
    print("[4/4] Seeding MongoDB...")
    mongo_js = generate_mongo_js()
    ok = run(
        ["docker", "exec", "-i", "infra-mongodb-1", "mongosh", "--quiet"],
        input_data=mongo_js,
        label="mongodb",
        timeout=300
    )
    if ok:
        print("  ✓ MongoDB done")

    print("\n=== All databases seeded ===")
    print(f"\nTables (each with {ROWS:,} rows):")
    print("  • users       — id, name, email, department, salary, status, is_verified, created_at")
    print("  • products    — id, name, category, price, stock, rating, is_active, created_at")
    print("  • orders      — id, user_id→users, status, total, created_at")
    print("  • order_items — id, order_id→orders, product_id→products, quantity, unit_price")


if __name__ == "__main__":
    main()
