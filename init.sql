-- Users table (existing)
CREATE TABLE users (
  id SERIAL PRIMARY KEY,
  name VARCHAR(50),
  email VARCHAR(100) UNIQUE,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  is_active BOOLEAN DEFAULT true
);

INSERT INTO users (name, email) VALUES
  ('Alpha', 'alpha@example.com'),
  ('Beta', 'beta@example.com'),
  ('Gamma', 'gamma@example.com'),
  ('Delta', 'delta@example.com'),
  ('Echo', 'echo@example.com'),
  ('Foxtrot', 'foxtrot@example.com');

-- Companies table
CREATE TABLE companies (
  id SERIAL PRIMARY KEY,
  name VARCHAR(100) NOT NULL,
  industry VARCHAR(50),
  founded_year INTEGER,
  headquarters VARCHAR(100),
  website VARCHAR(200),
  employee_count INTEGER,
  annual_revenue DECIMAL(15,2),
  is_public BOOLEAN DEFAULT false,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO companies (name, industry, founded_year, headquarters, website, employee_count, annual_revenue, is_public) VALUES
  ('TechCorp Solutions', 'Technology', 2010, 'San Francisco, CA', 'https://techcorp.com', 1250, 89500000.00, true),
  ('Global Manufacturing Inc', 'Manufacturing', 1985, 'Detroit, MI', 'https://globalmfg.com', 5600, 230000000.00, true),
  ('Green Energy Partners', 'Renewable Energy', 2015, 'Austin, TX', 'https://greenenergy.com', 340, 12800000.00, false),
  ('Digital Marketing Hub', 'Marketing', 2018, 'New York, NY', 'https://dmhub.com', 85, 5200000.00, false),
  ('Healthcare Innovations', 'Healthcare', 2005, 'Boston, MA', 'https://healthinnovate.com', 890, 45600000.00, false);

-- Categories table
CREATE TABLE categories (
  id SERIAL PRIMARY KEY,
  name VARCHAR(50) NOT NULL,
  description TEXT,
  parent_id INTEGER REFERENCES categories(id),
  sort_order INTEGER DEFAULT 0,
  is_active BOOLEAN DEFAULT true
);

INSERT INTO categories (name, description, parent_id, sort_order) VALUES
  ('Electronics', 'Electronic devices and components', NULL, 1),
  ('Computers', 'Desktop and laptop computers', 1, 1),
  ('Mobile Devices', 'Phones, tablets, and accessories', 1, 2),
  ('Home & Garden', 'Home improvement and gardening supplies', NULL, 2),
  ('Furniture', 'Indoor and outdoor furniture', 4, 1),
  ('Tools', 'Hand tools and power tools', 4, 2),
  ('Books', 'Physical and digital books', NULL, 3),
  ('Fiction', 'Novels and short stories', 7, 1),
  ('Non-Fiction', 'Educational and reference books', 7, 2);

-- Products table
CREATE TABLE products (
  id SERIAL PRIMARY KEY,
  sku VARCHAR(20) UNIQUE NOT NULL,
  name VARCHAR(100) NOT NULL,
  description TEXT,
  category_id INTEGER REFERENCES categories(id),
  price DECIMAL(10,2) NOT NULL,
  cost DECIMAL(10,2),
  stock_quantity INTEGER DEFAULT 0,
  min_stock_level INTEGER DEFAULT 5,
  weight_kg DECIMAL(6,2),
  dimensions_cm VARCHAR(20),
  manufacturer VARCHAR(50),
  warranty_months INTEGER DEFAULT 12,
  is_discontinued BOOLEAN DEFAULT false,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO products (sku, name, description, category_id, price, cost, stock_quantity, min_stock_level, weight_kg, dimensions_cm, manufacturer, warranty_months) VALUES
  ('LAPTOP001', 'UltraBook Pro 15', 'High-performance laptop with 16GB RAM and 512GB SSD', 2, 1299.99, 850.00, 25, 5, 1.8, '35x24x2', 'TechCorp', 24),
  ('PHONE001', 'SmartPhone X', 'Latest smartphone with advanced camera system', 3, 899.99, 600.00, 150, 20, 0.18, '15x7x1', 'MobileTech', 12),
  ('CHAIR001', 'Ergonomic Office Chair', 'Adjustable office chair with lumbar support', 5, 249.99, 125.00, 45, 10, 15.5, '60x60x120', 'ComfortSeating', 36),
  ('DRILL001', 'Cordless Power Drill', '18V cordless drill with 2 batteries', 6, 89.99, 45.00, 78, 15, 1.2, '25x8x20', 'PowerTools Pro', 24),
  ('BOOK001', 'The Art of Programming', 'Comprehensive guide to software development', 9, 49.99, 25.00, 200, 25, 0.8, '24x17x3', 'Tech Publishers', 0);

-- Order statuses table
CREATE TABLE order_statuses (
  id SERIAL PRIMARY KEY,
  name VARCHAR(30) NOT NULL UNIQUE,
  description VARCHAR(100),
  sort_order INTEGER DEFAULT 0
);

INSERT INTO order_statuses (name, description, sort_order) VALUES
  ('pending', 'Order received, awaiting processing', 1),
  ('processing', 'Order is being prepared', 2),
  ('shipped', 'Order has been shipped', 3),
  ('delivered', 'Order has been delivered', 4),
  ('cancelled', 'Order has been cancelled', 5),
  ('returned', 'Order has been returned', 6);

-- Orders table
CREATE TABLE orders (
  id SERIAL PRIMARY KEY,
  order_number VARCHAR(20) UNIQUE NOT NULL,
  user_id INTEGER REFERENCES users(id),
  company_id INTEGER REFERENCES companies(id),
  status_id INTEGER REFERENCES order_statuses(id) DEFAULT 1,
  order_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  ship_date TIMESTAMP,
  total_amount DECIMAL(12,2) NOT NULL,
  tax_amount DECIMAL(10,2) DEFAULT 0,
  shipping_amount DECIMAL(8,2) DEFAULT 0,
  discount_amount DECIMAL(10,2) DEFAULT 0,
  shipping_address TEXT,
  billing_address TEXT,
  notes TEXT
);

INSERT INTO orders (order_number, user_id, company_id, status_id, order_date, total_amount, tax_amount, shipping_amount, shipping_address, billing_address) VALUES
  ('ORD-2024-001', 1, 1, 3, '2024-01-15 10:30:00', 1549.98, 124.00, 25.99, '123 Main St, Anytown, ST 12345', '123 Main St, Anytown, ST 12345'),
  ('ORD-2024-002', 2, 2, 4, '2024-01-18 14:22:00', 899.99, 72.00, 15.99, '456 Oak Ave, Somewhere, ST 67890', '456 Oak Ave, Somewhere, ST 67890'),
  ('ORD-2024-003', 3, 1, 2, '2024-01-20 09:15:00', 339.97, 27.20, 12.99, '789 Pine Rd, Elsewhere, ST 54321', '789 Pine Rd, Elsewhere, ST 54321'),
  ('ORD-2024-004', 4, 3, 1, '2024-01-22 16:45:00', 139.98, 11.20, 8.99, '321 Elm St, Nowhere, ST 98765', '321 Elm St, Nowhere, ST 98765');

-- Order items table
CREATE TABLE order_items (
  id SERIAL PRIMARY KEY,
  order_id INTEGER REFERENCES orders(id) ON DELETE CASCADE,
  product_id INTEGER REFERENCES products(id),
  quantity INTEGER NOT NULL,
  unit_price DECIMAL(10,2) NOT NULL,
  total_price DECIMAL(12,2) NOT NULL,
  discount_percent DECIMAL(5,2) DEFAULT 0
);

INSERT INTO order_items (order_id, product_id, quantity, unit_price, total_price) VALUES
  (1, 1, 1, 1299.99, 1299.99),
  (1, 3, 1, 249.99, 249.99),
  (2, 2, 1, 899.99, 899.99),
  (3, 3, 1, 249.99, 249.99),
  (3, 4, 1, 89.99, 89.99),
  (4, 4, 1, 89.99, 89.99),
  (4, 5, 1, 49.99, 49.99);

-- User roles table
CREATE TABLE user_roles (
  id SERIAL PRIMARY KEY,
  name VARCHAR(30) NOT NULL UNIQUE,
  description VARCHAR(100),
  permissions TEXT[], -- Array of permissions
  is_active BOOLEAN DEFAULT true,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO user_roles (name, description, permissions) VALUES
  ('admin', 'Full system administrator', ARRAY['users.create', 'users.read', 'users.update', 'users.delete', 'orders.create', 'orders.read', 'orders.update', 'orders.delete']),
  ('manager', 'Department manager with limited admin rights', ARRAY['users.read', 'users.update', 'orders.read', 'orders.update', 'products.create', 'products.update']),
  ('employee', 'Regular employee access', ARRAY['orders.read', 'products.read', 'customers.read']),
  ('customer', 'Customer portal access', ARRAY['orders.read.own', 'profile.update']);

-- User role assignments (many-to-many)
CREATE TABLE user_role_assignments (
  id SERIAL PRIMARY KEY,
  user_id INTEGER REFERENCES users(id) ON DELETE CASCADE,
  role_id INTEGER REFERENCES user_roles(id) ON DELETE CASCADE,
  assigned_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  assigned_by INTEGER REFERENCES users(id),
  UNIQUE(user_id, role_id)
);

INSERT INTO user_role_assignments (user_id, role_id, assigned_by) VALUES
  (1, 1, 1), -- Alpha is admin
  (2, 2, 1), -- Beta is manager
  (3, 3, 1), -- Gamma is employee
  (4, 4, 1), -- Delta is customer
  (5, 3, 1), -- Echo is employee
  (6, 4, 1); -- Foxtrot is customer

-- Product reviews table
CREATE TABLE product_reviews (
  id SERIAL PRIMARY KEY,
  product_id INTEGER REFERENCES products(id) ON DELETE CASCADE,
  user_id INTEGER REFERENCES users(id),
  rating INTEGER CHECK (rating >= 1 AND rating <= 5),
  title VARCHAR(100),
  review_text TEXT,
  is_verified_purchase BOOLEAN DEFAULT false,
  helpful_votes INTEGER DEFAULT 0,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO product_reviews (product_id, user_id, rating, title, review_text, is_verified_purchase, helpful_votes) VALUES
  (1, 2, 5, 'Excellent laptop!', 'Great performance, long battery life, highly recommended for professional work.', true, 12),
  (1, 3, 4, 'Very good but pricey', 'Solid build quality and fast performance, but quite expensive.', true, 8),
  (2, 4, 5, 'Amazing camera quality', 'The camera on this phone is incredible, takes professional-quality photos.', true, 15),
  (3, 5, 4, 'Comfortable office chair', 'Very comfortable for long work sessions, good lumbar support.', true, 6),
  (4, 6, 5, 'Perfect for DIY projects', 'Powerful drill with long-lasting batteries, great value for money.', true, 9);

-- System logs table
CREATE TABLE system_logs (
  id SERIAL PRIMARY KEY,
  log_level VARCHAR(10) NOT NULL,
  message TEXT NOT NULL,
  module VARCHAR(50),
  user_id INTEGER REFERENCES users(id),
  ip_address INET,
  user_agent TEXT,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO system_logs (log_level, message, module, user_id, ip_address) VALUES
  ('INFO', 'User logged in successfully', 'authentication', 1, '192.168.1.100'),
  ('INFO', 'Order created successfully', 'orders', 2, '192.168.1.101'),
  ('WARNING', 'Failed login attempt', 'authentication', NULL, '192.168.1.102'),
  ('INFO', 'Product updated', 'products', 1, '192.168.1.100'),
  ('ERROR', 'Database connection timeout', 'database', NULL, NULL);

-- Create some indexes for better performance
CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_products_sku ON products(sku);
CREATE INDEX idx_products_category ON products(category_id);
CREATE INDEX idx_orders_user ON orders(user_id);
CREATE INDEX idx_orders_date ON orders(order_date);
CREATE INDEX idx_order_items_order ON order_items(order_id);
CREATE INDEX idx_order_items_product ON order_items(product_id);
CREATE INDEX idx_system_logs_created ON system_logs(created_at);
CREATE INDEX idx_system_logs_level ON system_logs(log_level);

-- Create a view for order summaries
CREATE VIEW order_summary AS
SELECT
  o.id,
  o.order_number,
  u.name as customer_name,
  u.email as customer_email,
  c.name as company_name,
  os.name as status,
  o.order_date,
  o.total_amount,
  COUNT(oi.id) as item_count
FROM orders o
JOIN users u ON o.user_id = u.id
LEFT JOIN companies c ON o.company_id = c.id
JOIN order_statuses os ON o.status_id = os.id
LEFT JOIN order_items oi ON o.id = oi.order_id
GROUP BY o.id, o.order_number, u.name, u.email, c.name, os.name, o.order_date, o.total_amount
ORDER BY o.order_date DESC;
