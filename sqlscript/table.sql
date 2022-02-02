CREATE TABLE IF NOT EXISTS binancebtctickernew(
    id SERIAL PRIMARY KEY
   ,e VARCHAR(14) NOT NULL
  ,TimeStamp_E BIGINT  NOT NULL
  ,s VARCHAR(7) NOT NULL
  ,c NUMERIC(50,8) NOT NULL
  ,o NUMERIC(50,8) NOT NULL
  ,h NUMERIC(50,8) NOT NULL
  ,l NUMERIC(50,8) NOT NULL
  ,v NUMERIC(150,8) NOT NULL
  ,q NUMERIC(150,8) NOT NULL
,topic VARCHAR(50) NOT NULL
,partition_msg BIGINT NOT NULL
,offset_msg BIGINT NOT NULL
);

CREATE TABLE newtraderorder(
   uuid               VARCHAR(100) NOT NULL PRIMARY KEY
  ,account_id         TEXT NOT NULL
  ,position_type      VARCHAR(50) NOT NULL
  -- ,position_side      INT  NOT NULL
  ,order_status       VARCHAR(50) NOT NULL
  ,order_type         VARCHAR(50) NOT NULL
  ,entryprice         NUMERIC NOT NULL
  ,execution_price    NUMERIC NOT NULL
  ,positionsize       NUMERIC NOT NULL
  ,leverage           NUMERIC NOT NULL
  ,initial_margin     NUMERIC NOT NULL
  ,available_margin   NUMERIC NOT NULL
  ,timestamp          bigint  NOT NULL
  ,bankruptcy_price   NUMERIC NOT NULL
  ,bankruptcy_value   NUMERIC NOT NULL
  ,maintenance_margin NUMERIC NOT NULL
  ,liquidation_price  NUMERIC NOT NULL
  ,unrealized_pnl     NUMERIC NOT NULL
  ,settlement_price   NUMERIC NOT NULL
  ,entry_nonce       bigint  NOT NULL
  ,exit_nonce        bigint  NOT NULL
  ,entry_sequence    bigint  NOT NULL
);
-- INSERT INTO testtable1(uuid,account_id,position_type,position_side,order_status,order_type,entryprice,execution_price,positionsize,leverage,initial_margin,available_margin,timestamp,bankruptcy_price,bankruptcy_value,maintenance_margin) VALUES ('1d5e4a52-5918-43ee-b8ed-dd2a3d89e34f',N'account_id','SHORT',1,'PENDING','MARKET',42514.01,0,3231277330.05,5,15201,15201,'1642155902808',53142.512500000004,60804,320.43708);

-- {"uuid":"41efbbcf-e65a-4fa2-b8f5-ce6f798d6f14","account_id":"account_id","position_type":"SHORT","position_side":1,"order_status":"PENDING","order_type":"MARKET","entryprice":42514.01,"execution_price":0.0,"positionsize":3231277330.05,"leverage":5.0,"initial_margin":15201.0,"available_margin":15201.0,"timestamp":1642155171428,"bankruptcy_price":53142.512500000004,"bankruptcy_value":60804.0,"maintenance_margin":320.43708}


-- pub struct LendOrder {
--     pub uuid: Uuid,
--     pub account_id: String,
--     pub balance: f64,
--     pub order_status: OrderStatus, //lend or settle
--     pub order_type: OrderType,     // LEND
--     pub nonce: i32,
--     pub deposit: f64,
--     pub new_lend_state_amount: f64,
--     pub timestamp: u128,
--     pub npoolshare: f64,
--     pub nwithdraw: f64,
--     pub payment: f64,
--     pub tlv0: f64, //total locked value before lend tx
--     pub tps0: f64, // total poolshare before lend tx
--     pub tlv1: f64, // total locked value after lend tx
--     pub tps1: f64, // total poolshre value after lend tx
--     pub tlv2: f64, // total locked value after lend payment/settlement
--     pub tps2: f64, // total poolshare after lend payment/settlement
-- }

CREATE TABLE newlendorder(
   uuid               VARCHAR(100) NOT NULL PRIMARY KEY
  ,account_id         TEXT NOT NULL
  ,balance            NUMERIC NOT NULL
  ,order_status       VARCHAR(50) NOT NULL
  ,order_type         VARCHAR(50) NOT NULL
  ,nonce              NUMERIC NOT NULL
  ,deposit            NUMERIC NOT NULL
  ,new_lend_state_amount   NUMERIC NOT NULL
  ,timestamp          bigint  NOT NULL
  ,npoolshare         NUMERIC NOT NULL
  ,nwithdraw          NUMERIC NOT NULL
  ,payment            NUMERIC NOT NULL
  ,tlv0               NUMERIC NOT NULL
  ,tps0               NUMERIC NOT NULL
  ,tlv1               NUMERIC NOT NULL
  ,tps1               NUMERIC NOT NULL
  ,tlv2               NUMERIC NOT NULL
  ,tps2               NUMERIC NOT NULL
);



--pendinglimittraderorder

CREATE TABLE pendinglimittraderorder(
   uuid               VARCHAR(100) NOT NULL PRIMARY KEY
  ,account_id         TEXT NOT NULL
  ,position_type      VARCHAR(50) NOT NULL
  -- ,position_side      INT  NOT NULL
  ,order_status       VARCHAR(50) NOT NULL
  ,order_type         VARCHAR(50) NOT NULL
  ,entryprice         NUMERIC NOT NULL
  ,execution_price    NUMERIC NOT NULL
  ,positionsize       NUMERIC NOT NULL
  ,leverage           NUMERIC NOT NULL
  ,initial_margin     NUMERIC NOT NULL
  ,available_margin   NUMERIC NOT NULL
  ,timestamp          bigint  NOT NULL
  ,bankruptcy_price   NUMERIC NOT NULL
  ,bankruptcy_value   NUMERIC NOT NULL
  ,maintenance_margin NUMERIC NOT NULL
  ,liquidation_price  NUMERIC NOT NULL
  ,unrealized_pnl     NUMERIC NOT NULL
  ,settlement_price   NUMERIC NOT NULL
  ,entry_nonce       bigint  NOT NULL
  ,exit_nonce        bigint  NOT NULL
  ,entry_sequence    bigint  NOT NULL
);