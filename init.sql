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

-- ============================================================================
-- ADVANCED DATA TYPE TESTING TABLE
-- This table contains various PostgreSQL data types for GUI testing
-- ============================================================================

-- Custom ENUM types
CREATE TYPE priority_level AS ENUM ('low', 'medium', 'high', 'critical');
CREATE TYPE task_status AS ENUM ('pending', 'in_progress', 'completed', 'cancelled');
CREATE TYPE mood_rating AS ENUM ('😀', '😊', '😐', '😢', '😡');

-- Composite type
CREATE TYPE address_type AS (
  street TEXT,
  city TEXT,
  state TEXT,
  zip_code TEXT,
  country TEXT
);

-- Advanced data types testing table
CREATE TABLE advanced_types_test (
  id SERIAL PRIMARY KEY,

  -- Numeric types
  tiny_int SMALLINT,
  regular_int INTEGER,
  big_int BIGINT,
  decimal_val DECIMAL(10,2),
  numeric_val NUMERIC(15,4),
  real_val REAL,
  double_val DOUBLE PRECISION,
  serial_val SERIAL,
  big_serial_val BIGSERIAL,

  -- Monetary type
  money_val MONEY,

  -- Character types
  char_fixed CHAR(10),
  varchar_var VARCHAR(100),
  text_unlimited TEXT,

  -- Binary data
  bytea_data BYTEA,

  -- Date/Time types
  date_val DATE,
  time_val TIME,
  time_tz_val TIME WITH TIME ZONE,
  timestamp_val TIMESTAMP,
  timestamp_tz_val TIMESTAMP WITH TIME ZONE,
  interval_val INTERVAL,

  -- Boolean
  bool_val BOOLEAN,

  -- Enumerated types
  priority priority_level,
  status task_status,
  mood mood_rating,

  -- Geometric types
  point_val POINT,
  line_val LINE,
  lseg_val LSEG,
  box_val BOX,
  path_val PATH,
  polygon_val POLYGON,
  circle_val CIRCLE,

  -- Network address types
  inet_val INET,
  cidr_val CIDR,
  macaddr_val MACADDR,
  macaddr8_val MACADDR8,

  -- Bit string types
  bit_val BIT(8),
  bit_varying_val BIT VARYING(16),

  -- Text search types
  tsvector_val TSVECTOR,
  tsquery_val TSQUERY,

  -- UUID type
  uuid_val UUID,

  -- XML type
  xml_val XML,

  -- JSON types
  json_val JSON,
  jsonb_val JSONB,

  -- Arrays
  int_array INTEGER[],
  text_array TEXT[],
  multi_dim_array INTEGER[][],

  -- Range types
  int_range INT4RANGE,
  bigint_range INT8RANGE,
  numeric_range NUMRANGE,
  timestamp_range TSRANGE,
  timestamptz_range TSTZRANGE,
  date_range DATERANGE,

  -- Object identifier types
  oid_val OID,
  regclass_val REGCLASS,
  regtype_val REGTYPE,
  regproc_val REGPROC,

  -- Composite type
  address address_type,

  -- pg_lsn (Log Sequence Number)
  lsn_val PG_LSN,

  -- Special types with NULL values for testing
  nullable_int INTEGER,
  nullable_text TEXT,
  nullable_json JSONB,

  -- Metadata
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  notes TEXT
);

-- Insert comprehensive test data
INSERT INTO advanced_types_test (
  tiny_int, regular_int, big_int, decimal_val, numeric_val, real_val, double_val,
  money_val,
  char_fixed, varchar_var, text_unlimited,
  bytea_data,
  date_val, time_val, time_tz_val, timestamp_val, timestamp_tz_val, interval_val,
  bool_val,
  priority, status, mood,
  point_val, line_val, lseg_val, box_val, path_val, polygon_val, circle_val,
  inet_val, cidr_val, macaddr_val, macaddr8_val,
  bit_val, bit_varying_val,
  tsvector_val, tsquery_val,
  uuid_val,
  xml_val,
  json_val, jsonb_val,
  int_array, text_array, multi_dim_array,
  int_range, bigint_range, numeric_range, timestamp_range, date_range,
  oid_val, regclass_val, regtype_val, regproc_val,
  address,
  lsn_val,
  nullable_int, nullable_text, nullable_json,
  notes
) VALUES
  -- Row 1: Comprehensive data
  (
    32767, 2147483647, 9223372036854775807, 12345.67, 9876543.2109, 3.14159, 2.718281828459045,
    '$1,234.56',
    'FIXED', 'Variable length string', 'This is unlimited text with special chars: üñíçödé 你好',
    '\xDEADBEEF'::bytea,
    '2024-01-15', '14:30:00', '14:30:00-05:00', '2024-01-15 14:30:00', '2024-01-15 14:30:00-05:00', '2 years 3 months 4 days 5 hours 6 minutes',
    true,
    'high', 'in_progress', '😊',
    '(1.5, 2.5)', '{1, 2, 3}', '[(0,0),(6,6)]', '((0,0),(3,3))', '((0,0),(1,1),(2,0))', '((0,0),(4,0),(4,4),(0,4))', '<(2,2),3>',
    '192.168.1.100', '192.168.0.0/24', '08:00:2b:01:02:03', '08:00:2b:01:02:03:04:05',
    B'10101010', B'1100110011001100',
    to_tsvector('english', 'The quick brown fox jumps over the lazy dog'),
    to_tsquery('english', 'quick & fox'),
    'a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11'::uuid,
    '<note><to>User</to><from>System</from><message>Test XML data</message></note>',
    '{"name": "John", "age": 30, "hobbies": ["reading", "coding"]}',
    '{"name": "Jane", "age": 25, "city": "NYC", "active": true, "metadata": {"role": "admin"}}',
    ARRAY[1, 2, 3, 4, 5], ARRAY['apple', 'banana', 'cherry'], ARRAY[[1,2,3],[4,5,6]],
    '[10,20)', '[100,200)', '[0.0,100.5)', '["2024-01-01 00:00:00","2024-12-31 23:59:59")', '["2024-01-01","2024-12-31")',
    16384, 'users'::regclass, 'integer'::regtype, 'sum'::regproc,
    ROW('123 Main St', 'New York', 'NY', '10001', 'USA')::address_type,
    '16/B374D848'::pg_lsn,
    42, 'Some text', '{"optional": "data"}',
    'Full featured test row with all types populated'
  ),
  -- Row 2: Edge cases and special values
  (
    -32768, -2147483648, -9223372036854775808, -99999.99, -1234.5678, -3.14, -2.71828,
    '-$999.99',
    'ABC', 'Short', 'Multi-line text
with line breaks
and special characters: @#$%^&*()',
    '\x00010203'::bytea,
    '1999-12-31', '23:59:59', '00:00:00+00:00', '1999-12-31 23:59:59', '1999-12-31 23:59:59+00:00', '0 seconds',
    false,
    'critical', 'pending', '😢',
    '(-1, -1)', '{-1, -2, -3}', '[(-5,-5),(-1,-1)]', '((-2,-2),(2,2))', '((-1,0),(0,1),(1,0))', '((-1,-1),(1,-1),(1,1),(-1,1))', '<(0,0),1>',
    '::1', '10.0.0.0/8', 'ff:ff:ff:ff:ff:ff', 'ff:ff:ff:ff:ff:ff:ff:ff',
    B'00000000', B'0',
    to_tsvector('simple', 'special characters: !@#$%'),
    to_tsquery('simple', 'special | characters'),
    '00000000-0000-0000-0000-000000000000'::uuid,
    '<empty/>',
    '{"empty": {}, "array": [], "null": null, "number": 0}',
    '{"unicode": "你好世界", "emoji": "🎉🎊", "nested": {"deep": {"value": 123}}}',
    ARRAY[]::INTEGER[], ARRAY['']::TEXT[], ARRAY[[0]]::INTEGER[][],
    '[,)', '[,]', '[,)', '(,)', '[,)',
    1, 'pg_type'::regclass, 'text'::regtype, 'avg'::regproc,
    ROW('', '', '', '', '')::address_type,
    '0/0'::pg_lsn,
    NULL, NULL, NULL,
    'Edge cases: negative numbers, empty values, special characters, unbounded ranges'
  ),
  -- Row 3: Alternative data for variety
  (
    100, 50000, 1000000, 999.99, 12.34, 1.5, 9.87654321,
    '$50.00',
    'TEST', 'Medium length varchar value', 'Plain text without special characters',
    '\xCAFEBABE'::bytea,
    '2023-06-15', '09:00:00', '12:00:00-07:00', '2023-06-15 09:00:00', '2023-06-15 12:00:00-07:00', '1 day 2 hours 30 minutes',
    true,
    'medium', 'completed', '😀',
    '(10, 20)', '{0, 0, 1}', '[(1,1),(10,10)]', '((5,5),(15,15))', '((0,0),(2,4),(4,0))', '((0,0),(5,0),(5,5),(0,5))', '<(5,5),2>',
    '10.0.0.1', '172.16.0.0/12', '00:11:22:33:44:55', '00:11:22:33:44:55:66:77',
    B'11111111', B'1010101010101010',
    to_tsvector('english', 'PostgreSQL database management system'),
    to_tsquery('english', 'PostgreSQL & database'),
    '550e8400-e29b-41d4-a716-446655440000'::uuid,
    '<data><item id="1">Value 1</item><item id="2">Value 2</item></data>',
    '{"status": "active", "count": 100}',
    '{"users": [{"id": 1, "name": "Alice"}, {"id": 2, "name": "Bob"}]}',
    ARRAY[10, 20, 30], ARRAY['red', 'green', 'blue'], ARRAY[[7,8,9],[10,11,12]],
    '[1,10]', '[1000,2000]', '[50.5,100.5]', '["2023-01-01 00:00:00","2023-06-30 23:59:59"]', '["2023-01-01","2023-06-30"]',
    32768, 'products'::regclass, 'boolean'::regtype, 'count'::regproc,
    ROW('456 Oak Ave', 'Los Angeles', 'CA', '90001', 'USA')::address_type,
    'FF/FFFFFFFF'::pg_lsn,
    0, '', '{}',
    'Alternative data set with different values'
  ),
  -- Row 4: More NULL values for testing
  (
    NULL, NULL, NULL, NULL, NULL, NULL, NULL,
    NULL,
    NULL, NULL, NULL,
    NULL,
    NULL, NULL, NULL, NULL, NULL, NULL,
    NULL,
    'low', 'cancelled', '😐',
    NULL, NULL, NULL, NULL, NULL, NULL, NULL,
    NULL, NULL, NULL, NULL,
    NULL, NULL,
    NULL, NULL,
    NULL,
    NULL,
    NULL, NULL,
    NULL, NULL, NULL,
    NULL, NULL, NULL, NULL, NULL,
    NULL, NULL, NULL, NULL,
    NULL,
    NULL,
    NULL, NULL, NULL,
    'Row with mostly NULL values to test NULL handling in GUI'
  ),
  -- Row 5: Mixed case with emoji mood
  (
    255, 65535, 2147483647, 100.00, 200.50, 0.5, 1.234567,
    '$0.01',
    'MINIMAL', 'Test', 'Single line text',
    '\xFF'::bytea,
    CURRENT_DATE, CURRENT_TIME, CURRENT_TIME, NOW(), NOW(), '15 minutes',
    false,
    'low', 'pending', '😡',
    '(0, 0)', '{1, 1, 1}', '[(0,0),(1,1)]', '((0,0),(1,1))', '((0,0),(1,0),(0.5,1))', '((0,0),(1,0),(1,1),(0,1))', '<(1,1),0.5>',
    '127.0.0.1', '192.168.1.0/24', 'aa:bb:cc:dd:ee:ff', 'aa:bb:cc:dd:ee:ff:00:11',
    B'01010101', B'110011',
    to_tsvector('english', 'test data'),
    to_tsquery('english', 'test'),
    gen_random_uuid(),
    '<simple>text</simple>',
    '{"simple": "json"}',
    '{"test": true}',
    ARRAY[0], ARRAY['single'], ARRAY[[1]],
    '[-10,10]', '[-100,100]', '[-1,1]', '["2024-01-01","2024-01-02"]', '["2024-01-01","2024-01-01"]',
    100, 'categories'::regclass, 'varchar'::regtype, 'min'::regproc,
    ROW('789 Pine St', 'Chicago', 'IL', '60601', 'USA')::address_type,
    '1/12345678'::pg_lsn,
    1, 'test', '{"key": "value"}',
    'Minimal values with current date/time functions'
  );

-- Create indexes for testing
CREATE INDEX idx_advanced_priority ON advanced_types_test(priority);
CREATE INDEX idx_advanced_status ON advanced_types_test(status);
CREATE INDEX idx_advanced_date ON advanced_types_test(date_val);
CREATE INDEX idx_advanced_jsonb ON advanced_types_test USING gin(jsonb_val);
CREATE INDEX idx_advanced_tsvector ON advanced_types_test USING gin(tsvector_val);

-- Create remaining original indexes
CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_products_sku ON products(sku);
CREATE INDEX idx_products_category ON products(category_id);
CREATE INDEX idx_orders_user ON orders(user_id);
CREATE INDEX idx_orders_date ON orders(order_date);
CREATE INDEX idx_order_items_order ON order_items(order_id);
CREATE INDEX idx_order_items_product ON order_items(product_id);
CREATE INDEX idx_system_logs_created ON system_logs(created_at);
CREATE INDEX idx_system_logs_level ON system_logs(log_level);

-- Create a view for the advanced types test to make it easier to query
CREATE VIEW advanced_types_summary AS
SELECT
  id,
  priority,
  status,
  mood,
  bool_val,
  date_val,
  jsonb_val,
  int_array,
  text_array,
  inet_val,
  uuid_val,
  notes
FROM advanced_types_test
ORDER BY id;
