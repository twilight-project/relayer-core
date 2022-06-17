-- Recent Order Table
CREATE TABLE 'recentorders' (
  side INT,
  price DOUBLE,
  amount DOUBLE,
  timestamp TIMESTAMP
) timestamp (timestamp) PARTITION BY DAY;


-- Candal Data Query
-- Select t3.*, t4.buy_volume from ( 
Select t3.timestamp,t3.open,t3.close,t3.min,t3.max,coalesce(t3.sell_volume , 0), coalesce(t4.buy_volume , 0)from ( 
  Select t1.*,t2.sell_volume from ( 
      SELECT timestamp, first(price) AS open, last(price) AS close, min(price), max(price) 
          FROM recentorders WHERE timestamp > dateadd('d', -1, now())
            SAMPLE BY 1m ALIGN TO CALENDAR) t1 

      LEFT OUTER JOIN (

      SELECT timestamp,sum(amount) AS sell_volume 
          FROM recentorders WHERE side=0 AND timestamp > dateadd('d', -1, now())
                SAMPLE BY 1m ALIGN TO CALENDAR) t2 ON t1.timestamp = t2.timestamp ) t3 
LEFT OUTER JOIN ( 

SELECT timestamp, sum(amount) AS buy_volume 
      FROM recentorders WHERE side=1 AND timestamp > dateadd('d', -1, now())
            SAMPLE BY 1m ALIGN TO CALENDAR) t4 ON t3.timestamp = t4.timestamp;